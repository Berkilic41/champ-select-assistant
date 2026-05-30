use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::num::NonZeroU32;
use std::sync::Arc;

pub type Limiter = Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>;

pub fn new_limiter(per_second: u32) -> Limiter {
    // Clamp to ≥1 to avoid panic when callers pass 0 (governor requires NonZeroU32).
    let rate = NonZeroU32::new(per_second.max(1)).expect("per_second.max(1) is always non-zero");
    let quota = Quota::per_second(rate);
    Arc::new(RateLimiter::direct(quota))
}
