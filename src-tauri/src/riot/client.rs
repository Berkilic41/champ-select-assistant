use super::rate_limiter::{new_limiter, Limiter};
use anyhow::{bail, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug)]
pub struct RiotClient {
    api_key: String,
    proxy_base: Option<String>,
    http: Client,
    limiter: Limiter,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SummonerInfo {
    pub puuid: String,
    pub game_name: String,
    pub tag_line: String,
    pub region: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncResult {
    pub synced: u32,
    pub skipped: u32,
    pub errors: u32,
}

// ChampionStats is now a shared pure DTO in `csa-core`. Re-exported.
pub use csa_core::types::ChampionStats;

/// Maps a platform region (e.g. "tr1") to a regional routing value (e.g. "europe").
pub fn routing_for_region(region: &str) -> &'static str {
    match region.to_lowercase().as_str() {
        "euw1" | "eune1" | "tr1" | "ru" => "europe",
        "na1" | "la1" | "la2" => "americas",
        "kr" | "jp1" => "asia",
        _ => "europe",
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn find_env_file_from(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .take(8)
        .map(|dir| dir.join(".env"))
        .find(|path| path.is_file())
}

/// Reloads the nearest `.env` file so long-running dev sessions can pick up a
/// rotated Riot key without restarting the Tauri process.
pub fn reload_runtime_env() -> Option<PathBuf> {
    let candidates = [
        std::env::current_dir().ok(),
        std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf)),
    ];

    for start in candidates.into_iter().flatten() {
        if let Some(path) = find_env_file_from(&start) {
            if let Err(err) = dotenvy::from_path_override(&path) {
                tracing::warn!("Runtime .env reload failed ({}): {err}", path.display());
            }
            return Some(path);
        }
    }

    None
}

/// Builds a fresh Riot client from the current runtime environment.
///
/// This intentionally reloads `.env` every time. A dev Riot key expires often;
/// keeping the client static would force users to restart the app after key
/// rotation.
pub fn runtime_client_from_env() -> Option<Arc<RiotClient>> {
    reload_runtime_env();

    if let Some(proxy_url) = non_empty_env("PROXY_URL") {
        Some(Arc::new(RiotClient::new_with_proxy(proxy_url)))
    } else {
        non_empty_env("RIOT_API_KEY").map(|key| Arc::new(RiotClient::new(key)))
    }
}

pub fn runtime_riot_configured() -> bool {
    runtime_client_from_env().is_some()
}

impl RiotClient {
    pub fn new(api_key: String) -> Self {
        let http = Client::builder()
            .use_rustls_tls()
            .build()
            .expect("reqwest client oluşturulamadı");
        RiotClient {
            api_key,
            proxy_base: None,
            http,
            limiter: new_limiter(20),
        }
    }

    pub fn new_with_proxy(proxy_base: String) -> Self {
        let http = Client::builder()
            .use_rustls_tls()
            .build()
            .expect("reqwest client oluşturulamadı");
        RiotClient {
            api_key: String::new(),
            proxy_base: Some(proxy_base),
            http,
            limiter: new_limiter(20),
        }
    }

    fn effective_url(&self, url: &str) -> String {
        match &self.proxy_base {
            Some(base) => {
                // "https://tr1.api.riotgames.com/lol/..." → "{base}/tr1.api.riotgames.com/lol/..."
                let stripped = url.strip_prefix("https://").unwrap_or(url);
                format!("{}/{}", base.trim_end_matches('/'), stripped)
            }
            None => url.to_string(),
        }
    }

    /// Single authenticated GET (header attached unless behind a proxy).
    async fn send_get(&self, url: &str) -> Result<reqwest::Response> {
        let mut builder = self.http.get(url);
        if self.proxy_base.is_none() {
            builder = builder.header("X-Riot-Token", &self.api_key);
        }
        Ok(builder.send().await?)
    }

    pub async fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        self.limiter.until_ready().await;
        let effective = self.effective_url(url);
        let resp = self.send_get(&effective).await?;

        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            let wait = resp
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(5);
            // Honor Retry-After + a little jitter so retries don't synchronize.
            tokio::time::sleep(Duration::from_secs(wait) + Duration::from_millis(jitter_ms(250)))
                .await;
            self.limiter.until_ready().await;
            let resp2 = self.send_get(&effective).await?;
            if !resp2.status().is_success() {
                bail!("Riot API error {} (after retry): {}", resp2.status(), url);
            }
            return Ok(resp2.json::<T>().await?);
        }

        // Transient server error (5xx) → one jittered backoff retry before failing.
        if resp.status().is_server_error() {
            tokio::time::sleep(backoff_delay(0, jitter_ms(250))).await;
            self.limiter.until_ready().await;
            let resp2 = self.send_get(&effective).await?;
            if !resp2.status().is_success() {
                bail!(
                    "Riot API error {} (after 5xx retry): {}",
                    resp2.status(),
                    url
                );
            }
            return Ok(resp2.json::<T>().await?);
        }

        if !resp.status().is_success() {
            bail!("Riot API error {}: {}", resp.status(), url);
        }

        Ok(resp.json::<T>().await?)
    }
}

/// Exponential backoff with caller-supplied jitter (pure, testable). Base 250 ms,
/// doubling per attempt, capped at 4 s, plus `jitter_ms`.
fn backoff_delay(attempt: u32, jitter_ms: u64) -> Duration {
    let base = 250u64.saturating_mul(1u64 << attempt.min(4)).min(4000);
    Duration::from_millis(base + jitter_ms)
}

/// Small non-cryptographic jitter in `0..max` derived from the clock — avoids a
/// `rand` dependency and keeps retries from synchronizing into a thundering herd.
fn jitter_ms(max: u64) -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::from(d.subsec_nanos()) % max.max(1))
        .unwrap_or(0)
}

#[cfg(test)]
mod backoff_tests {
    use super::*;

    #[test]
    fn backoff_delay_doubles_and_caps() {
        assert_eq!(backoff_delay(0, 0).as_millis(), 250);
        assert_eq!(backoff_delay(1, 0).as_millis(), 500);
        assert_eq!(backoff_delay(2, 0).as_millis(), 1000);
        assert_eq!(backoff_delay(4, 0).as_millis(), 4000);
        assert_eq!(backoff_delay(9, 0).as_millis(), 4000, "capped at 4s");
        assert_eq!(backoff_delay(0, 100).as_millis(), 350, "jitter added");
    }

    #[test]
    fn jitter_is_within_bounds() {
        for _ in 0..50 {
            assert!(jitter_ms(250) < 250);
        }
        assert_eq!(jitter_ms(0), 0, "no panic on zero");
    }
}
