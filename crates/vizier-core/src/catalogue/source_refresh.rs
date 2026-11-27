//! The one refresh policy every catalogue source follows (spec 068, data-model §9): due after
//! `catalogue.ttl`, a 1 h backoff after a failure, one check in flight, and a manual refresh
//! that waits for a running check instead of starting a second one.
//!
//! **Only people's reads start a check** (FR-011): the callers are the `/catalogue/*`,
//! `/providers` and `/providers/custom` handlers in `service/catalogue.rs`, never agent,
//! scheduler or dream code.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use parking_lot::Mutex;

use super::RefreshStatus;
use crate::config::CatalogueConfig;
use crate::time::Timestamp;

/// How long a manual refresh waits for a check already running.
pub const WAIT: Duration = Duration::from_secs(30);
const FAILURE_BACKOFF: chrono::Duration = chrono::Duration::hours(1);

#[derive(Default)]
pub struct SourceRefresh {
    in_progress: AtomicBool,
    /// The last failed check, for the backoff and the status line; restored at startup.
    failure: Mutex<Option<(String, Timestamp)>>,
}

impl SourceRefresh {
    /// Whether a background check should start now: refresh on, the data older than the TTL,
    /// no failure in the last hour. Does not claim the check; see [`Self::try_begin`].
    pub fn is_due(&self, config: &CatalogueConfig, checked: Timestamp) -> bool {
        if !config.refresh {
            return false;
        }
        let ttl = chrono::Duration::from_std(*config.ttl).unwrap_or(chrono::Duration::hours(24));
        let now = crate::time::now();
        if now - checked < ttl {
            return false;
        }
        if let Some((_, at)) = *self.failure.lock()
            && now - at < FAILURE_BACKOFF
        {
            return false;
        }
        !self.in_progress.load(Ordering::SeqCst)
    }

    /// Claim the one check in flight; `false` when another is running.
    pub fn try_begin(&self) -> bool {
        !self.in_progress.swap(true, Ordering::SeqCst)
    }

    pub fn end(&self) {
        self.in_progress.store(false, Ordering::SeqCst);
    }

    pub fn in_progress(&self) -> bool {
        self.in_progress.load(Ordering::SeqCst)
    }

    /// Wait (up to [`WAIT`]) for a running check, then report how it ended.
    pub async fn wait(&self) -> Result<(), String> {
        let deadline = tokio::time::Instant::now() + WAIT;
        while self.in_progress() && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        match self.failure() {
            Some((e, _)) => Err(e),
            None => Ok(()),
        }
    }

    pub fn failure(&self) -> Option<(String, Timestamp)> {
        self.failure.lock().clone()
    }

    pub fn set_failure(&self, error: String, at: Timestamp) {
        *self.failure.lock() = Some((error, at));
    }

    pub fn clear_failure(&self) {
        *self.failure.lock() = None;
    }

    pub fn status(&self, config: &CatalogueConfig) -> RefreshStatus {
        let failure = self.failure();
        RefreshStatus {
            enabled: config.refresh,
            ttl: config.ttl.to_string(),
            in_progress: self.in_progress(),
            last_error: failure.as_ref().map(|(e, _)| e.clone()),
            failed_at: failure.map(|(_, at)| at),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_check_at_a_time_and_a_failure_backs_off() {
        let r = SourceRefresh::default();
        let config = CatalogueConfig::default();
        let old = crate::time::epoch();
        assert!(r.is_due(&config, old));
        assert!(r.try_begin());
        assert!(!r.try_begin(), "a second check is refused");
        assert!(!r.is_due(&config, old), "not due while one runs");
        r.end();
        r.set_failure("boom".into(), crate::time::now());
        assert!(!r.is_due(&config, old), "backs off after a failure");
        r.clear_failure();
        assert!(r.is_due(&config, old));
        assert!(!r.is_due(&config, crate::time::now()), "fresh data is not due");
        let off = CatalogueConfig {
            refresh: false,
            ..CatalogueConfig::default()
        };
        assert!(!r.is_due(&off, old));
    }
}
