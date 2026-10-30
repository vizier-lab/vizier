//! A small sliding-window limiter for client events and saved state
//! (`specs/019-client-tools` research R15). It lives on `HTTPState`, so a limit holds across
//! every socket a person has open. The `VizierDependencies.side_chat_limits` instance is
//! shared with the Discord and Telegram readers the same way (`specs/033-btw-side-chat`
//! research R9), which is what makes one limit hold per platform identity across channels.

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

    /// How much of the window is left before `key` fits again, when it is over the limit.
    /// Reads without counting a hit, so a refusal never spends the budget it denies
    /// (`specs/033-btw-side-chat` T012: the reader shows this as "try again in {n}s").
    pub fn retry_after(&self, key: &str) -> Option<Duration> {
        let now = Instant::now();
        let mut hits = self.hits.lock().ok()?;
        hits.retain(|_, times| {
            while times.front().is_some_and(|t| now.duration_since(*t) >= self.window) {
                times.pop_front();
            }
            !times.is_empty()
        });
        let times = hits.get(key)?;
        if times.len() < self.max {
            return None;
        }
        let oldest = *times.front()?;
        Some(self.window.saturating_sub(now.duration_since(oldest)))
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
