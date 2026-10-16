//! One channel reader kept running by its own small supervisor (#50), used by
//! `ChannelConnections` for every connection it runs.
//!
//! A reader whose `run` ends, with an error or without one, is started again after a
//! backoff, the same as an agent that stops: a dropped gateway or a network outage
//! recovers by itself. An error retrying can't fix ([`FatalChannelError`]: a revoked token,
//! intents the bot isn't allowed) marks the reader failed and leaves it stopped, because the
//! fix is a new token, and saving one starts the reader afresh.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use chrono::Utc;
use tokio::time::Instant;

use crate::agents::supervisor::{HEALTHY_RUN, restart_delay};
use crate::channels::{FatalChannelError, VizierChannel};
use crate::schema::{ChannelHealth, ChannelPlatform, ChannelState};

/// One reader's health, as its supervisor last reported it.
pub type HealthSlot = Arc<RwLock<ChannelHealth>>;

pub type Reader = Arc<dyn VizierChannel + Send + Sync>;

/// Run one reader until `stopping` is set, restarting it with backoff whenever it ends.
pub async fn supervise(
    agent_id: String,
    platform: ChannelPlatform,
    reader: Reader,
    stopping: Arc<AtomicBool>,
    health: HealthSlot,
) {
    let mut failures: u32 = 0;
    loop {
        let started = Instant::now();
        let result = reader.run().await;
        if stopping.load(Ordering::SeqCst) {
            return;
        }
        if started.elapsed() >= HEALTHY_RUN {
            failures = 0;
        }
        failures += 1;
        let error = match result {
            Ok(()) => anyhow::anyhow!("the reader stopped on its own"),
            Err(e) => e,
        };

        if error.downcast_ref::<FatalChannelError>().is_some() {
            tracing::error!(
                agent_id,
                %platform,
                "{platform} reader failed and won't be retried until its token is replaced: {error:#}"
            );
            set(&health, |h| {
                h.state = ChannelState::Failed;
                h.last_error = Some(format!("{error:#}"));
                h.failures = failures;
                h.next_retry_at = None;
            });
            return;
        }

        let wait = restart_delay(failures);
        tracing::warn!(
            agent_id,
            %platform,
            failures,
            retry_in_secs = wait.as_secs(),
            "{platform} reader stopped, reconnecting: {error:#}"
        );
        set(&health, |h| {
            h.state = ChannelState::Reconnecting;
            h.last_error = Some(format!("{error:#}"));
            h.failures = failures;
            h.next_retry_at = chrono::Duration::from_std(wait).ok().map(|w| Utc::now() + w);
        });
        tokio::time::sleep(wait).await;
        if stopping.load(Ordering::SeqCst) {
            return;
        }
        tracing::info!(agent_id, %platform, "restarting {platform} reader");
        set(&health, |h| {
            h.state = ChannelState::Running;
            h.next_retry_at = None;
        });
    }
}

fn set(health: &HealthSlot, f: impl FnOnce(&mut ChannelHealth)) {
    f(&mut health.write().unwrap_or_else(|p| p.into_inner()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;

    /// Fails `fail_times` times with `error`, then stays running.
    struct FlakyReader {
        runs: AtomicU32,
        fail_times: u32,
        fatal: bool,
    }

    #[async_trait::async_trait]
    impl VizierChannel for FlakyReader {
        async fn run(&self) -> anyhow::Result<()> {
            let run = self.runs.fetch_add(1, Ordering::SeqCst);
            if run < self.fail_times {
                if self.fatal {
                    return Err(FatalChannelError("bad token".into()).into());
                }
                return Err(anyhow::anyhow!("gateway closed"));
            }
            std::future::pending().await
        }
    }

    fn health(platform: ChannelPlatform) -> HealthSlot {
        Arc::new(RwLock::new(ChannelHealth {
            platform,
            state: ChannelState::Running,
            last_error: None,
            failures: 0,
            next_retry_at: None,
        }))
    }

    #[tokio::test(start_paused = true)]
    async fn a_reader_that_stops_is_restarted() {
        let reader = Arc::new(FlakyReader { runs: AtomicU32::new(0), fail_times: 2, fatal: false });
        let health = health(ChannelPlatform::Discord);
        let task = tokio::spawn(supervise(
            "agent".into(),
            ChannelPlatform::Discord,
            reader.clone(),
            Arc::new(AtomicBool::new(false)),
            health.clone(),
        ));

        tokio::time::sleep(Duration::from_secs(1)).await;
        {
            let h = &health.read().unwrap();
            assert_eq!(h.state, ChannelState::Reconnecting);
            assert_eq!(h.last_error.as_deref(), Some("gateway closed"));
        }

        // 2s, then 4s, of backoff.
        tokio::time::sleep(Duration::from_secs(10)).await;
        assert_eq!(reader.runs.load(Ordering::SeqCst), 3);
        let h = health.read().unwrap().clone();
        assert_eq!(h.state, ChannelState::Running);
        assert_eq!(h.failures, 2, "kept until it has run for a while");
        assert!(!task.is_finished());
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_fatal_error_marks_the_reader_failed_and_stops_retrying() {
        let reader = Arc::new(FlakyReader { runs: AtomicU32::new(0), fail_times: 99, fatal: true });
        let health = health(ChannelPlatform::Telegram);
        let task = tokio::spawn(supervise(
            "agent".into(),
            ChannelPlatform::Telegram,
            reader.clone(),
            Arc::new(AtomicBool::new(false)),
            health.clone(),
        ));

        tokio::time::sleep(Duration::from_secs(600)).await;
        assert!(task.is_finished());
        assert_eq!(reader.runs.load(Ordering::SeqCst), 1);
        let h = health.read().unwrap().clone();
        assert_eq!(h.state, ChannelState::Failed);
        assert_eq!(h.last_error.as_deref(), Some("bad token"));
    }

    #[tokio::test(start_paused = true)]
    async fn a_reader_ending_during_a_stop_is_not_restarted() {
        let reader = Arc::new(FlakyReader { runs: AtomicU32::new(0), fail_times: 99, fatal: false });
        let stopping = Arc::new(AtomicBool::new(true));
        supervise(
            "agent".into(),
            ChannelPlatform::Discord,
            reader.clone(),
            stopping,
            health(ChannelPlatform::Discord),
        )
        .await;
        assert_eq!(reader.runs.load(Ordering::SeqCst), 1);
    }
}
