use anyhow::Result;

use crate::{
    schema::{AgentId, TopicId, VizierChannelId, VizierSessionDetail},
    storage::VizierStorage,
};

/// One conversation a person could open (spec 024, conversation picker). A Discord or Telegram
/// channel is one conversation; a WebUI channel holds several, one per topic the person made, and
/// each topic is its own row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationRow {
    /// The channel slug: `discord__123`, `telegram__-9`, `http__ana__vizier-webui`.
    pub channel: String,
    pub topics: u64,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// The topic with the newest message ("" for a channel without topics).
    pub latest_topic: String,
    /// Where the latest topic falls among the topics, oldest first, from 1.
    pub latest_ordinal: u64,
    /// The latest topic's title, when it has one.
    pub title: Option<String>,
}

impl ConversationRow {
    /// Orders rows that moved at the same instant, and names a row in a cursor: a WebUI
    /// conversation is a topic, anywhere else it is the channel.
    pub fn key(&self) -> String {
        if self.channel.starts_with("http__") { format!("{}#{}", self.channel, self.latest_topic) } else { self.channel.clone() }
    }
}

/// Only the newest this many history rows are searched for message text.
pub const CONVERSATION_SEARCH_ROWS: usize = 5000;

#[async_trait::async_trait]
pub trait SessionStorage {
    /// The agent's conversations on channels people talk on, newest first, each once. With `q`,
    /// only those whose title contains it (any topic's) or that have it in a message among the
    /// agent's newest [`CONVERSATION_SEARCH_ROWS`], case-insensitively.
    async fn conversation_rows(&self, agent_id: &str, q: Option<&str>) -> Result<Vec<ConversationRow>> {
        let _ = (agent_id, q);
        Ok(vec![])
    }

    async fn save_session_detail(&self, session: VizierSessionDetail) -> Result<()>;
    async fn update_session_detail(&self, session: VizierSessionDetail) -> Result<()>;
    async fn update_thinking_state(
        &self,
        agent_id: AgentId,
        chanel: VizierChannelId,
        topic: Option<TopicId>,
        is_thinking: bool,
    ) -> Result<()>;
    /// Startup: no turn is running yet, so every session still marked thinking is a leftover,
    /// except those in `keep`, whose turn is about to resume.
    async fn reset_thinking_except(&self, keep: &[crate::schema::VizierSession]) -> Result<usize> {
        let _ = keep;
        Ok(0)
    }
    /// The agents with a turn running now (a session marked thinking).
    async fn thinking_agents(&self) -> Result<std::collections::HashSet<String>> {
        Ok(Default::default())
    }

    async fn get_session_detail_by_topic(
        &self,
        agent_id: AgentId,
        chanel: VizierChannelId,
        topic: Option<TopicId>,
    ) -> Result<Option<VizierSessionDetail>>;

    async fn get_session_list(
        &self,
        agent_id: AgentId,
        chanel: Option<VizierChannelId>,
    ) -> Result<Vec<VizierSessionDetail>>;

    async fn delete_session(
        &self,
        agent_id: AgentId,
        chanel: VizierChannelId,
        topic: TopicId,
    ) -> Result<()>;
}

#[async_trait::async_trait]
impl SessionStorage for VizierStorage {
    async fn conversation_rows(&self, agent_id: &str, q: Option<&str>) -> Result<Vec<ConversationRow>> {
        self.0.conversation_rows(agent_id, q).await
    }

    async fn thinking_agents(&self) -> Result<std::collections::HashSet<String>> {
        self.0.thinking_agents().await
    }

    async fn reset_thinking_except(&self, keep: &[crate::schema::VizierSession]) -> Result<usize> {
        self.0.reset_thinking_except(keep).await
    }

    async fn save_session_detail(&self, session: VizierSessionDetail) -> Result<()> {
        self.0.save_session_detail(session).await
    }

    async fn update_session_detail(&self, session: VizierSessionDetail) -> Result<()> {
        self.0.update_session_detail(session).await
    }

    async fn update_thinking_state(
        &self,
        agent_id: AgentId,
        chanel: VizierChannelId,
        topic: Option<TopicId>,
        is_thinking: bool,
    ) -> Result<()> {
        self.0
            .update_thinking_state(agent_id, chanel, topic, is_thinking)
            .await
    }

    async fn get_session_detail_by_topic(
        &self,
        agent_id: AgentId,
        chanel: VizierChannelId,
        topic: Option<TopicId>,
    ) -> Result<Option<VizierSessionDetail>> {
        self.0
            .get_session_detail_by_topic(agent_id, chanel, topic)
            .await
    }

    async fn get_session_list(
        &self,
        agent_id: AgentId,
        chanel: Option<VizierChannelId>,
    ) -> Result<Vec<VizierSessionDetail>> {
        self.0.get_session_list(agent_id, chanel).await
    }

    async fn delete_session(
        &self,
        agent_id: AgentId,
        chanel: VizierChannelId,
        topic: TopicId,
    ) -> Result<()> {
        self.0.delete_session(agent_id, chanel, topic).await
    }
}
