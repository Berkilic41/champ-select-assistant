use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::collections::VecDeque;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub type Limiter = Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>;

pub fn new_limiter(per_second: u32) -> Limiter {
    // Clamp to ≥1 to avoid panic when callers pass 0 (governor requires NonZeroU32).
    let rate = NonZeroU32::new(per_second.max(1)).expect("per_second.max(1) is always non-zero");
    let quota = Quota::per_second(rate);
    Arc::new(RateLimiter::direct(quota))
}

// ── Rolling request budget ──────────────────────────────────────────────────
// The `governor` limiter above caps the *burst rate* (req/sec). Riot personal
// dev keys have a second, binding ceiling: **100 requests / 2 minutes**. The
// governor does not enforce that rolling-count limit, so under aggressive
// Match-V5 collection a single tick could, in the worst case, exceed it and earn
// a 429 — which risks key revocation (= the whole data pipeline dies).
//
// `RollingBudget` is a simple sliding-window counter: callers `try_acquire()`
// before each Riot HTTP call and stop early when the window is full. This keeps
// aggressive growth *sustainable* by riding just under Riot's real ceiling.
pub const RIOT_BUDGET_WINDOW_SECS: u64 = 120;
/// Stay safely under Riot's 100 req / 2 min personal-key ceiling.
pub const RIOT_BUDGET_MAX: usize = 85;

pub struct RollingBudget {
    window: Duration,
    max: usize,
    hits: Mutex<VecDeque<Instant>>,
}

impl RollingBudget {
    pub fn new(window: Duration, max: usize) -> Self {
        Self {
            window,
            max,
            hits: Mutex::new(VecDeque::new()),
        }
    }

    /// Records a request and returns `true` if the budget allowed it, or `false`
    /// when the rolling window is already full (caller should back off).
    pub fn try_acquire(&self) -> bool {
        self.try_acquire_at(Instant::now())
    }

    /// Testable core: drops timestamps older than `window`, then admits one
    /// request iff fewer than `max` remain in the window.
    pub fn try_acquire_at(&self, now: Instant) -> bool {
        let mut hits = self.hits.lock().expect("rolling budget mutex poisoned");
        while let Some(&front) = hits.front() {
            if now.duration_since(front) >= self.window {
                hits.pop_front();
            } else {
                break;
            }
        }
        if hits.len() >= self.max {
            return false;
        }
        hits.push_back(now);
        true
    }

    /// Remaining capacity in the window as of `now` (for diagnostics/tests).
    #[allow(dead_code)]
    pub fn remaining_at(&self, now: Instant) -> usize {
        let mut hits = self.hits.lock().expect("rolling budget mutex poisoned");
        while let Some(&front) = hits.front() {
            if now.duration_since(front) >= self.window {
                hits.pop_front();
            } else {
                break;
            }
        }
        self.max.saturating_sub(hits.len())
    }
}

/// Process-global Riot request budget (one Riot key → one shared ceiling).
pub fn riot_budget() -> &'static RollingBudget {
    static BUDGET: OnceLock<RollingBudget> = OnceLock::new();
    BUDGET.get_or_init(|| {
        RollingBudget::new(
            Duration::from_secs(RIOT_BUDGET_WINDOW_SECS),
            RIOT_BUDGET_MAX,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admits_up_to_max_then_blocks_within_window() {
        let budget = RollingBudget::new(Duration::from_secs(120), 3);
        let t0 = Instant::now();
        assert!(budget.try_acquire_at(t0));
        assert!(budget.try_acquire_at(t0 + Duration::from_secs(1)));
        assert!(budget.try_acquire_at(t0 + Duration::from_secs(2)));
        // Fourth call inside the 120s window is refused — no 429 reaches Riot.
        assert!(!budget.try_acquire_at(t0 + Duration::from_secs(3)));
        assert_eq!(budget.remaining_at(t0 + Duration::from_secs(3)), 0);
    }

    #[test]
    fn window_slides_so_old_hits_free_capacity() {
        let budget = RollingBudget::new(Duration::from_secs(120), 2);
        let t0 = Instant::now();
        assert!(budget.try_acquire_at(t0));
        assert!(budget.try_acquire_at(t0 + Duration::from_secs(10)));
        assert!(!budget.try_acquire_at(t0 + Duration::from_secs(20)));
        // Once the first hit ages out past the 120s window, capacity returns.
        assert!(budget.try_acquire_at(t0 + Duration::from_secs(121)));
    }

    #[test]
    fn riot_budget_is_configured_under_riot_ceiling() {
        // Defense-in-depth: our cap must stay under Riot's 100 req / 2 min.
        assert!(RIOT_BUDGET_MAX < 100);
        assert_eq!(RIOT_BUDGET_WINDOW_SECS, 120);
        let b = riot_budget();
        assert!(b.try_acquire());
    }
}
