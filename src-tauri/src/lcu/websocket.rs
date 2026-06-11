// Sprint F: Hardened WebSocket champ-select watcher.
//
// - Graceful shutdown via `tokio_util::sync::CancellationToken`.
// - Exponential backoff with jitter (2s → 4s → 8s → 16s → 30s cap).
// - Auth (401) failure short-circuit: no infinite reconnect on bad creds.
// - rustls-based TLS that accepts the LCU's self-signed certificate.

use std::sync::Arc;

use anyhow::Result;
use base64::{engine::general_purpose, Engine};
use futures_util::{SinkExt, StreamExt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use tauri::{AppHandle, Emitter};
use tokio::time::{sleep, timeout, Duration};
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{http::StatusCode, Error as WsError, Message},
    Connector,
};
use tokio_util::sync::CancellationToken;

use super::lockfile::Lockfile;
use super::session::parse_session;

/// rustls verifier that accepts any server certificate.
///
/// SAFETY: The LCU listens on 127.0.0.1 with a per-launch self-signed cert.
/// We're not talking to a remote attacker — we're talking to a local socket
/// whose Basic-auth token (from the lockfile) already authenticates the peer.
/// Skipping cert verification here is the standard pattern for LCU clients.
#[derive(Debug)]
struct AcceptAllVerifier;

impl ServerCertVerifier for AcceptAllVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}

fn build_rustls_config() -> rustls::ClientConfig {
    // rustls 0.23 requires an explicit crypto provider. Use aws-lc-rs (the
    // default feature) and install it for this config only — without touching
    // the process-wide default that other code in the binary may rely on.
    let provider = rustls::crypto::aws_lc_rs::default_provider();
    rustls::ClientConfig::builder_with_provider(Arc::new(provider))
        .with_safe_default_protocol_versions()
        .expect("rustls: TLS protocol versions must initialize")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAllVerifier))
        .with_no_client_auth()
}

/// Deterministic jitter (no `rand` dep): up to ±500ms variation.
fn jitter_ms(attempt: u32) -> u64 {
    // Map attempt count to a pseudo-random offset in [0, 1000) ms.
    (u64::from(attempt).wrapping_mul(137)) % 1000
}

fn backoff_delay(consecutive_errors: u32) -> Duration {
    // 2s, 4s, 8s, 16s, 30s cap. min(4) ensures the shift never exceeds 1<<4=16.
    let base_ms = 2_000u64.saturating_mul(1u64 << consecutive_errors.min(4));
    let capped = base_ms.min(30_000);
    Duration::from_millis(capped + jitter_ms(consecutive_errors))
}

/// Detect an HTTP 401 surfaced by tungstenite during the WebSocket upgrade.
fn is_auth_failure(err: &WsError) -> bool {
    matches!(err, WsError::Http(resp) if resp.status() == StatusCode::UNAUTHORIZED)
}

/// Spawns a reconnecting WebSocket loop that emits `champ-select-session`
/// events to the Tauri frontend whenever the LCU sends a session update.
///
/// Cancellation: callers pass a `CancellationToken`; cancelling it triggers
/// a graceful shutdown (no more reconnect attempts).
pub async fn start_champ_select_watcher(app: AppHandle, lf: Lockfile, cancel: CancellationToken) {
    let url = format!("wss://127.0.0.1:{}/", lf.port);
    let auth = general_purpose::STANDARD.encode(format!("riot:{}", lf.password));

    let mut consecutive_errors: u32 = 0;

    loop {
        // Race the WS loop against cancellation so we shut down promptly.
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                tracing::info!("WS watcher graceful shutdown (pre-connect)");
                return;
            }
            res = run_ws_loop(&url, &auth, &app, &cancel) => {
                match res {
                    Ok(()) => {
                        tracing::info!("WS bağlantısı normal kapandı");
                        return;
                    }
                    Err(e) => {
                        if let Some(ws_err) = e.downcast_ref::<WsError>() {
                            if is_auth_failure(ws_err) {
                                tracing::error!(
                                    "WS auth hatası (401), yeniden başlatılmayacak: {e}"
                                );
                                return;
                            }
                        }
                        consecutive_errors = consecutive_errors.saturating_add(1);
                        let delay = backoff_delay(consecutive_errors);
                        tracing::warn!(
                            "WS bağlantısı kesildi: {e} — {}ms sonra yeniden deneniyor (deneme #{})",
                            delay.as_millis(),
                            consecutive_errors
                        );

                        tokio::select! {
                            biased;
                            _ = cancel.cancelled() => {
                                tracing::info!("WS watcher graceful shutdown (during backoff)");
                                return;
                            }
                            _ = sleep(delay) => {}
                        }
                    }
                }
            }
        }
    }
}

async fn run_ws_loop(
    url: &str,
    auth: &str,
    app: &AppHandle,
    cancel: &CancellationToken,
) -> Result<()> {
    let rustls_config = build_rustls_config();
    let connector = Connector::Rustls(Arc::new(rustls_config));

    // Build the HTTP upgrade request with the Basic-auth header
    let request = {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut req = url.into_client_request()?;
        req.headers_mut().insert(
            tokio_tungstenite::tungstenite::http::header::AUTHORIZATION,
            format!("Basic {auth}").parse()?,
        );
        req
    };

    // tokio-tungstenite returns `Err(WsError::Http(_))` directly for any
    // non-101 response (including 401 Unauthorized), so we don't need to
    // re-check the status here. The caller's `is_auth_failure(&WsError)`
    // catches that case from the propagated error.
    // Timeout the TLS handshake: a half-open LCU socket must not hang the watcher
    // forever. On elapse we bail → the caller's backoff reconnects.
    let (mut ws, _response) = match timeout(
        Duration::from_secs(5),
        connect_async_tls_with_config(request, None, false, Some(connector)),
    )
    .await
    {
        Ok(result) => result?,
        Err(_) => anyhow::bail!("LCU WebSocket bağlantısı 5s içinde kurulamadı"),
    };

    tracing::info!("LCU WS bağlantısı kuruldu");

    // WAMP subscribe to the champ-select session endpoint
    let subscribe =
        serde_json::json!([5, "OnJsonApiEvent_lol-champ-select_v1_session"]).to_string();
    ws.send(Message::Text(subscribe)).await?;

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                tracing::info!("WS run_ws_loop: cancel sinyali alındı, kapatılıyor");
                let _ = ws.send(Message::Close(None)).await;
                return Ok(());
            }
            msg = ws.next() => {
                let Some(msg) = msg else { break };
                match msg? {
                    Message::Text(text) => {
                        if let Ok(arr) = serde_json::from_str::<serde_json::Value>(&text) {
                            // WAMP event type 8 = EVENT
                            if arr[0].as_u64() == Some(8) {
                                let data = &arr[2]["data"];
                                if data.is_null() {
                                    let _ = app.emit(
                                        "champ-select-session",
                                        serde_json::Value::Null,
                                    );
                                } else if let Some(state) = parse_session(data) {
                                    let _ = app.emit("champ-select-session", &state);
                                }
                            }
                        }
                    }
                    Message::Close(_) => break,
                    Message::Ping(payload) => {
                        ws.send(Message::Pong(payload)).await?;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_then_caps() {
        // Base ms values (without jitter) — verify the 30s cap & growth.
        assert!(backoff_delay(0).as_millis() >= 2_000);
        assert!(backoff_delay(0).as_millis() < 3_000); // 2s + jitter <1s
        assert!(backoff_delay(1).as_millis() >= 4_000);
        assert!(backoff_delay(2).as_millis() >= 8_000);
        assert!(backoff_delay(3).as_millis() >= 16_000);
        // shift saturates at 4 → 32s base, capped to 30s
        let capped = backoff_delay(10);
        assert!(capped.as_millis() >= 30_000);
        assert!(capped.as_millis() < 31_000); // 30s + jitter <1s
    }

    #[test]
    fn jitter_is_bounded() {
        for i in 0u32..50 {
            assert!(jitter_ms(i) < 1_000);
        }
    }

    // -------------------------------------------------------------------------
    // Reconnect backoff: delay strictly increases with consecutive error count
    // -------------------------------------------------------------------------

    #[test]
    fn backoff_delay_increases_with_consecutive_errors() {
        // Compare base delays (strip jitter by looking at multiples of 1000ms).
        // backoff_delay(n) base = 2^(n+1) seconds, cap 30s.
        // We verify the *minimum* (base without jitter) grows.
        let d0 = backoff_delay(0).as_millis();
        let d1 = backoff_delay(1).as_millis();
        let d2 = backoff_delay(2).as_millis();
        let d3 = backoff_delay(3).as_millis();

        // Each step must add at least 1 second to the base
        assert!(
            d1 > d0,
            "delay after 1 error must exceed delay after 0 errors"
        );
        assert!(
            d2 > d1,
            "delay after 2 errors must exceed delay after 1 error"
        );
        assert!(
            d3 > d2,
            "delay after 3 errors must exceed delay after 2 errors"
        );
    }

    #[test]
    fn backoff_delay_caps_at_thirty_seconds() {
        // After the cap is reached, no further growth (modulo jitter)
        let capped_a = backoff_delay(5).as_millis();
        let capped_b = backoff_delay(100).as_millis();
        // Both must be in the [30_000, 31_000) range
        assert!(capped_a >= 30_000, "cap lower bound: {capped_a}");
        assert!(capped_a < 31_000, "cap upper bound: {capped_a}");
        assert!(capped_b >= 30_000, "cap lower bound (large n): {capped_b}");
        assert!(capped_b < 31_000, "cap upper bound (large n): {capped_b}");
    }

    // -------------------------------------------------------------------------
    // CancellationToken: cancel() → watcher must stop
    // -------------------------------------------------------------------------

    #[test]
    fn cancellation_token_cancel_flags_as_cancelled() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled(), "fresh token must not be cancelled");
        token.cancel();
        assert!(
            token.is_cancelled(),
            "token must be cancelled after .cancel()"
        );
    }

    #[test]
    fn child_token_is_cancelled_when_parent_cancelled() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        assert!(!child.is_cancelled());
        parent.cancel();
        assert!(
            child.is_cancelled(),
            "child token must be cancelled when parent is cancelled"
        );
    }

    #[tokio::test]
    async fn cancelled_token_future_resolves_immediately() {
        let token = CancellationToken::new();
        token.cancel();

        // Must complete in well under 50ms — if not, something is wrong
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(50), token.cancelled()).await;

        assert!(
            result.is_ok(),
            "cancelled() future must resolve immediately on an already-cancelled token"
        );
    }

    /// Verifies that a `tokio::select!` branch on a pre-cancelled token is taken
    /// immediately, which is the exact pattern used inside `start_champ_select_watcher`.
    /// Tests the cancellation-select logic in isolation without requiring a live AppHandle.
    #[tokio::test]
    async fn select_biased_prefers_cancelled_token_over_slow_future() {
        let token = CancellationToken::new();
        token.cancel(); // cancel before the select

        // Replicate the pattern from start_champ_select_watcher:
        //   biased select: cancelled() branch vs a 10s sleep.
        // With a pre-cancelled token the cancelled() branch must win immediately.
        let outcome = tokio::time::timeout(std::time::Duration::from_millis(200), async {
            let mut shutdown_triggered = false;
            tokio::select! {
                biased;
                _ = token.cancelled() => { shutdown_triggered = true; }
                _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {}
            }
            shutdown_triggered
        })
        .await;

        assert!(
            outcome.is_ok(),
            "select must complete within 200ms for a pre-cancelled token"
        );
        assert!(
            outcome.unwrap(),
            "the cancellation branch must have been taken, not the slow-sleep branch"
        );
    }

    /// Verifies that cancelling a token mid-backoff interrupts the sleep.
    #[tokio::test]
    async fn cancel_during_backoff_sleep_resolves_immediately() {
        let token = CancellationToken::new();
        let token_clone = token.clone();

        // Spawn a task that cancels the token after a short delay
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            token_clone.cancel();
        });

        // Simulate the backoff-sleep select from the watcher loop
        let outcome = tokio::time::timeout(std::time::Duration::from_millis(200), async {
            let mut cancelled_during_backoff = false;
            tokio::select! {
                biased;
                _ = token.cancelled() => { cancelled_during_backoff = true; }
                _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {}
            }
            cancelled_during_backoff
        })
        .await;

        assert!(
            outcome.is_ok(),
            "cancel must interrupt a long backoff sleep within 200ms"
        );
        assert!(
            outcome.unwrap(),
            "the cancellation branch must be taken when token is cancelled during backoff"
        );
    }
}
