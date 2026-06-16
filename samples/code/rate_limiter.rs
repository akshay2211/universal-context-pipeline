//! Token-bucket rate limiter.
//!
//! Each `RateLimiter` represents a single bucket with a fixed capacity and a
//! refill rate measured in tokens per second. `try_acquire` is non-blocking;
//! `acquire` waits until enough tokens are available.

use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    capacity: f64,
    refill_rate_per_sec: f64,
    state: Mutex<BucketState>,
}

struct BucketState {
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    pub fn new(capacity: u32, refill_rate_per_sec: f64) -> Self {
        Self {
            capacity: capacity as f64,
            refill_rate_per_sec,
            state: Mutex::new(BucketState {
                tokens: capacity as f64,
                last_refill: Instant::now(),
            }),
        }
    }

    /// Try to consume `n` tokens without blocking. Returns false when the
    /// bucket doesn't have enough tokens.
    pub fn try_acquire(&self, n: u32) -> bool {
        let mut state = self.state.lock().unwrap();
        self.refill(&mut state);
        let want = n as f64;
        if state.tokens >= want {
            state.tokens -= want;
            true
        } else {
            false
        }
    }

    /// Block (sleeping) until `n` tokens become available, then consume them.
    pub fn acquire(&self, n: u32) {
        loop {
            if self.try_acquire(n) {
                return;
            }
            // Worst case: wait for the time it would take to refill the deficit.
            let deficit = (n as f64) - self.state.lock().unwrap().tokens;
            let secs = deficit / self.refill_rate_per_sec;
            std::thread::sleep(Duration::from_secs_f64(secs.max(0.001)));
        }
    }

    fn refill(&self, state: &mut BucketState) {
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        let added = elapsed * self.refill_rate_per_sec;
        state.tokens = (state.tokens + added).min(self.capacity);
        state.last_refill = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_full() {
        let rl = RateLimiter::new(5, 1.0);
        for _ in 0..5 {
            assert!(rl.try_acquire(1));
        }
        assert!(!rl.try_acquire(1), "bucket should be empty");
    }

    #[test]
    fn refills_over_time() {
        let rl = RateLimiter::new(2, 10.0);
        assert!(rl.try_acquire(2));
        assert!(!rl.try_acquire(1));
        std::thread::sleep(Duration::from_millis(150));
        assert!(rl.try_acquire(1));
    }
}
