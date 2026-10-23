use anyhow::Result;

use crate::{
    schema::{AgentId, TopicId, VizierChannelId, VizierSessionDetail},
    storage::VizierStorage,
};

#[async_trait::async_trait]
pub trait SessionStorage {
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
