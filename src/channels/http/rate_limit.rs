//! A small sliding-window limiter for client events and saved state
//! (`specs/019-client-tools` research R15). It lives on `HTTPState`, so a limit holds across
//! every socket a person has open.

use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

pub struct RateLimiter {
    max: usize,
    window: Duration,
    hits: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl RateLimiter {
    pub fn new(max: usize, window: Duration) -> Self {
        Self {
            max,
            window,
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// Count one hit for `key`; `false` (and nothing counted) when it is over the limit.
    pub fn check(&self, key: &str) -> bool {
        self.check_at(key, Instant::now())
    }

    fn check_at(&self, key: &str, now: Instant) -> bool {
        let Ok(mut hits) = self.hits.lock() else {
            return true;
        };
        // Expired instants go on every access, and keys left empty with them, so the map
        // only ever holds keys active within the last window.
        hits.retain(|_, times| {
            while times.front().is_some_and(|t| now.duration_since(*t) >= self.window) {
                times.pop_front();
            }
            !times.is_empty()
        });
        let times = hits.entry(key.to_string()).or_default();
        if times.len() >= self.max {
            return false;
        }
        times.push_back(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_the_limit_within_a_window() {
        let limiter = RateLimiter::new(2, Duration::from_secs(1));
        let t0 = Instant::now();
        assert!(limiter.check_at("a", t0));
        assert!(limiter.check_at("a", t0 + Duration::from_millis(100)));
        assert!(!limiter.check_at("a", t0 + Duration::from_millis(200)));
        // Another key has its own budget.
        assert!(limiter.check_at("b", t0 + Duration::from_millis(200)));
    }

    #[test]
    fn the_window_slides() {
        let limiter = RateLimiter::new(2, Duration::from_secs(1));
        let t0 = Instant::now();
        assert!(limiter.check_at("a", t0));
        assert!(limiter.check_at("a", t0 + Duration::from_millis(500)));
        assert!(!limiter.check_at("a", t0 + Duration::from_millis(900)));
        // The first hit has left the window; the second has not.
        assert!(limiter.check_at("a", t0 + Duration::from_millis(1000)));
        assert!(!limiter.check_at("a", t0 + Duration::from_millis(1100)));
    }

    #[test]
    fn empty_keys_are_pruned() {
        let limiter = RateLimiter::new(1, Duration::from_secs(1));
        let t0 = Instant::now();
        limiter.check_at("a", t0);
        limiter.check_at("b", t0 + Duration::from_secs(2));
        let hits = limiter.hits.lock().unwrap();
        assert!(!hits.contains_key("a"));
        assert!(hits.contains_key("b"));
    }
}
