use std::sync::Arc;

use anyhow::Result;

use crate::{channels::connections::ChannelConnections, dependencies::VizierDependencies};

pub mod commands;
pub mod connections;
pub mod discord;
pub mod http;
pub mod reactions;
pub mod readers;
pub mod side_chat;
pub mod telegram;
pub mod woken;

/// A reader error that retrying won't fix, such as a revoked token: the reader is marked
/// failed rather than restarted, until its connection's token is replaced.
#[derive(Debug)]
pub struct FatalChannelError(pub String);

impl std::fmt::Display for FatalChannelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FatalChannelError {}

#[async_trait::async_trait]
pub trait VizierChannel {
    async fn run(&self) -> Result<()>;
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

/// Every channel connection's reader (Discord, Telegram), reconciled against storage at start
/// and on every channel change. The HTTP server is not one of them: its host starts it.
pub struct ChannelReaders {
    connections: Arc<ChannelConnections>,
}

impl ChannelReaders {
    pub fn new(deps: VizierDependencies) -> Self {
        Self {
            connections: Arc::new(ChannelConnections::new(deps)),
        }
    }

    /// Starts the readers and keeps them in step with storage. Never returns while healthy:
    /// a subsystem that returns is a stop (#51).
    pub async fn run(&self) -> Result<()> {
        self.connections.reconcile().await;
        self.connections.clone().watch().await;
        Err(anyhow::anyhow!("the channel watcher stopped"))
    }

    /// Stops every reader. Aborting [`ChannelReaders::run`] doesn't: each reader runs in a task
    /// of its own.
    pub async fn shutdown(&self) {
        self.connections.shutdown_all().await;
    }
}

/// The note put ahead of a reply to a turn that vizier's restart interrupted
/// (`specs/023-persist-turn-activity`).
pub fn continued_note(resumed: bool, content: String) -> String {
    if resumed {
        format!("_continued after a restart_\n\n{content}")
    } else {
        content
    }
}

/// The one short notice a channel sends after a reply's text when an attachment cannot be
/// resolved or delivered (`specs/035-add-artifacts` FR-053): never silent, never a lost reply.
pub fn attachment_failure_notice(filename: &str) -> String {
    format!("[Attachment '{filename}' could not be sent]")
}

#[cfg(test)]
mod continued_tests {
    use super::continued_note;

    #[test]
    fn only_a_resumed_reply_is_marked() {
        assert_eq!(continued_note(false, "hi".into()), "hi");
        assert!(continued_note(true, "hi".into()).starts_with("_continued after a restart_"));
    }
}
