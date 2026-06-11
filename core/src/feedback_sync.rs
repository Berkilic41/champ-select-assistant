//! Feedback sync policy — pure helpers (Feedback Loop — Sprint D, Claude).
//!
//! The local `recommendation_feedback` queue is offline-first: rows are written
//! with `synced_at IS NULL` and flushed to `/v1/recommendation-feedback` when
//! online. This module holds the PURE policy the (future / Codex-owned) flush
//! command needs — retry backoff, a stable idempotency key to prevent duplicate
//! ingestion, and the attempt cap. No IO here; see docs/audit/feedback-sync-analytics.md.
#![allow(dead_code)] // consumed by the flush command wiring (Codex/coordinated)

/// First retry delay.
const RETRY_BASE_SECS: u64 = 30;
/// Backoff ceiling — never wait longer than this between retries.
const RETRY_MAX_SECS: u64 = 3600;
/// Give up (mark `failed`, stop auto-retrying) after this many attempts.
pub const MAX_ATTEMPTS: u32 = 6;

/// Exponential backoff in seconds for the Nth retry (0-based): 30, 60, 120, …
/// capped at `RETRY_MAX_SECS`. Pure + overflow-safe.
pub fn retry_backoff_secs(attempt: u32) -> u64 {
    if attempt >= 7 {
        return RETRY_MAX_SECS;
    }
    (RETRY_BASE_SECS << attempt).min(RETRY_MAX_SECS)
}

/// Absolute unix-second timestamp of the next retry given the current time.
pub fn next_retry_at(now: i64, attempt: u32) -> i64 {
    now + retry_backoff_secs(attempt) as i64
}

/// Whether another automatic retry should be scheduled. After `MAX_ATTEMPTS` the
/// row is terminal-`failed` and waits for a manual / next-session flush.
pub fn should_retry(attempt: u32) -> bool {
    attempt < MAX_ATTEMPTS
}

/// Stable idempotency key for a logical feedback event. The flush sends this so the
/// backend can dedupe (unique index) — re-sending the same row (crash mid-flush,
/// double-tap) is then a no-op instead of a duplicate. Deterministic FNV-1a hex; a
/// dedup key, NOT a security hash. `user_hash` is already a hash on the client side.
pub fn idempotency_key(
    user_hash: &str,
    champion_id: u32,
    verdict: &str,
    created_at: i64,
) -> String {
    let composite = format!("{user_hash}|{champion_id}|{verdict}|{created_at}");
    format!("{:016x}", fnv1a64(&composite))
}

fn fnv1a64(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// One queued row's sync bookkeeping (mirrors the V013 columns).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlushState {
    pub synced_at: Option<i64>,
    pub retry_count: u32,
    pub last_error: Option<String>,
    pub next_retry_at: Option<i64>,
}

/// Outcome of one POST attempt the flush command feeds back to the resolver.
pub enum SendResult {
    Ok,
    Failed(String),
}

/// A row is due for a flush attempt when it is not yet synced and its backoff has
/// elapsed (nothing scheduled, or the scheduled time has passed).
pub fn is_due(state: &FlushState, now: i64) -> bool {
    state.synced_at.is_none() && state.next_retry_at.map(|t| t <= now).unwrap_or(true)
}

/// Apply a send result to a row's sync state.
///
/// **Correctness property (network failure never corrupts the queue):** a row is
/// marked `synced` ONLY on success. A failure leaves `synced_at` null and just
/// bumps `retry_count` / `last_error` / `next_retry_at` — the feedback is never
/// dropped. After `MAX_ATTEMPTS` failures the row is terminal-`failed`
/// (`next_retry_at = None`) and waits for a manual / next-session flush.
pub fn resolve_after_send(prev: &FlushState, result: SendResult, now: i64) -> FlushState {
    match result {
        SendResult::Ok => FlushState {
            synced_at: Some(now),
            retry_count: prev.retry_count,
            last_error: None,
            next_retry_at: None,
        },
        SendResult::Failed(err) => {
            let retry_count = prev.retry_count + 1;
            let next_retry_at = if should_retry(retry_count) {
                Some(next_retry_at(now, retry_count - 1))
            } else {
                None
            };
            FlushState {
                synced_at: None,
                retry_count,
                last_error: Some(err),
                next_retry_at,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_exponential_then_capped() {
        assert_eq!(retry_backoff_secs(0), 30);
        assert_eq!(retry_backoff_secs(1), 60);
        assert_eq!(retry_backoff_secs(2), 120);
        assert_eq!(retry_backoff_secs(3), 240);
        // Caps and never overflows for large/absurd attempt counts.
        assert_eq!(retry_backoff_secs(7), RETRY_MAX_SECS);
        assert_eq!(retry_backoff_secs(1000), RETRY_MAX_SECS);
    }

    #[test]
    fn backoff_is_monotonic_up_to_cap() {
        let mut prev = 0;
        for attempt in 0..10 {
            let b = retry_backoff_secs(attempt);
            assert!(b >= prev, "backoff must not decrease");
            assert!(b <= RETRY_MAX_SECS);
            prev = b;
        }
    }

    #[test]
    fn next_retry_at_adds_backoff() {
        assert_eq!(next_retry_at(1_000, 0), 1_030);
        assert_eq!(next_retry_at(1_000, 2), 1_120);
    }

    #[test]
    fn should_retry_stops_at_cap() {
        assert!(should_retry(0));
        assert!(should_retry(MAX_ATTEMPTS - 1));
        assert!(!should_retry(MAX_ATTEMPTS));
        assert!(!should_retry(MAX_ATTEMPTS + 5));
    }

    #[test]
    fn idempotency_key_is_stable_and_distinct() {
        let a = idempotency_key("uhash", 238, "helpful", 1_800_000_000);
        let again = idempotency_key("uhash", 238, "helpful", 1_800_000_000);
        assert_eq!(a, again, "same logical event → same key");

        // Any field change → different key.
        assert_ne!(a, idempotency_key("other", 238, "helpful", 1_800_000_000));
        assert_ne!(a, idempotency_key("uhash", 99, "helpful", 1_800_000_000));
        assert_ne!(
            a,
            idempotency_key("uhash", 238, "not_helpful", 1_800_000_000)
        );
        assert_ne!(a, idempotency_key("uhash", 238, "helpful", 1_800_000_001));
        assert_eq!(a.len(), 16, "compact 64-bit hex key");
    }

    fn pending() -> FlushState {
        FlushState {
            synced_at: None,
            retry_count: 0,
            last_error: None,
            next_retry_at: None,
        }
    }

    #[test]
    fn success_marks_synced_and_clears_error() {
        let out = resolve_after_send(&pending(), SendResult::Ok, 5_000);
        assert_eq!(out.synced_at, Some(5_000));
        assert_eq!(out.last_error, None);
        assert_eq!(out.next_retry_at, None);
    }

    #[test]
    fn failure_never_marks_synced_and_schedules_backoff() {
        let out = resolve_after_send(&pending(), SendResult::Failed("timeout".into()), 5_000);
        assert_eq!(out.synced_at, None, "failure must NOT drop/sync the row");
        assert_eq!(out.retry_count, 1);
        assert_eq!(out.last_error.as_deref(), Some("timeout"));
        assert_eq!(out.next_retry_at, Some(5_030), "first retry after 30s");
    }

    #[test]
    fn repeated_failures_become_terminal_after_max_attempts() {
        let mut state = pending();
        for _ in 0..MAX_ATTEMPTS {
            state = resolve_after_send(&state, SendResult::Failed("err".into()), 1_000);
        }
        assert_eq!(state.retry_count, MAX_ATTEMPTS);
        assert_eq!(state.synced_at, None, "still queued, never lost");
        assert_eq!(
            state.next_retry_at, None,
            "terminal failed — no auto retry scheduled"
        );
    }

    #[test]
    fn is_due_respects_sync_and_backoff() {
        assert!(is_due(&pending(), 100), "unsynced + unscheduled → due");
        let scheduled = FlushState {
            next_retry_at: Some(200),
            ..pending()
        };
        assert!(!is_due(&scheduled, 100), "backoff not elapsed → not due");
        assert!(is_due(&scheduled, 200), "backoff elapsed → due");
        let synced = FlushState {
            synced_at: Some(50),
            ..pending()
        };
        assert!(!is_due(&synced, 1_000), "already synced → never due");
    }
}
