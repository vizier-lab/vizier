use anyhow::Result;

use crate::{
    schema::{ReactionEntry, SessionHistory},
    storage::VizierStorage,
};

/// A platform the agent posts to under a bot account of its own, whose message ids are
/// linked back to the history entry they render. A storage key, not something dispatch
/// branches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Discord,
    Telegram,
}

impl Platform {
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::Discord => "discord",
            Platform::Telegram => "telegram",
        }
    }
}

/// Who reacted, in the channel's own id space. `name` is the display name at the time of
/// reacting, when the channel could resolve one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reactor {
    pub id: String,
    pub name: Option<String>,
}

/// Reactions on agent replies (`specs/013-reaction-awareness/data-model.md`), and the links
/// from the platform messages an agent posted to the history entry each one renders.
#[async_trait::async_trait]
pub trait ReactionStorage {
    /// A re-add is a no-op that keeps the original `added_at`.
    async fn add_reaction(&self, history_uid: &str, reactor: &Reactor, emoji: &str) -> Result<()>;
    /// Removing an absent reaction is a no-op, never an add.
    async fn remove_reaction(&self, history_uid: &str, reactor_id: &str, emoji: &str)
    -> Result<()>;
    /// `None` clears every emoji.
    async fn clear_reactions(&self, history_uid: &str, emoji: Option<&str>) -> Result<()>;
    /// Oldest first.
    async fn list_reactions(&self, history_uid: &str) -> Result<Vec<ReactionEntry>>;

    async fn link_platform_messages(
        &self,
        agent_id: &str,
        platform: Platform,
        chat_id: &str,
        message_ids: &[String],
        history_uid: &str,
    ) -> Result<()>;
    async fn find_linked_message(
        &self,
        agent_id: &str,
        platform: Platform,
        chat_id: &str,
        message_id: &str,
    ) -> Result<Option<SessionHistory>>;

    /// For a WebUI target: the entry, so the caller can check it is an agent `Response` and
    /// recover its session for the broadcast.
    async fn get_history_entry(&self, history_uid: &str) -> Result<Option<SessionHistory>>;
}

#[async_trait::async_trait]
impl ReactionStorage for VizierStorage {
    async fn add_reaction(&self, history_uid: &str, reactor: &Reactor, emoji: &str) -> Result<()> {
        self.0.add_reaction(history_uid, reactor, emoji).await
    }

    async fn remove_reaction(
        &self,
        history_uid: &str,
        reactor_id: &str,
        emoji: &str,
    ) -> Result<()> {
        self.0.remove_reaction(history_uid, reactor_id, emoji).await
    }

    async fn clear_reactions(&self, history_uid: &str, emoji: Option<&str>) -> Result<()> {
        self.0.clear_reactions(history_uid, emoji).await
    }

    async fn list_reactions(&self, history_uid: &str) -> Result<Vec<ReactionEntry>> {
        self.0.list_reactions(history_uid).await
    }

    async fn link_platform_messages(
        &self,
        agent_id: &str,
        platform: Platform,
        chat_id: &str,
        message_ids: &[String],
        history_uid: &str,
    ) -> Result<()> {
        self.0
            .link_platform_messages(agent_id, platform, chat_id, message_ids, history_uid)
            .await
    }

    async fn find_linked_message(
        &self,
        agent_id: &str,
        platform: Platform,
        chat_id: &str,
        message_id: &str,
    ) -> Result<Option<SessionHistory>> {
        self.0
            .find_linked_message(agent_id, platform, chat_id, message_id)
            .await
    }

    async fn get_history_entry(&self, history_uid: &str) -> Result<Option<SessionHistory>> {
        self.0.get_history_entry(history_uid).await
    }
}
