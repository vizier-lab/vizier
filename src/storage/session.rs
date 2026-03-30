use anyhow::Result;

use crate::{
    schema::{AgentId, TopicId, VizierChannelId, VizierSessionDetail},
    storage::VizierStorage,
};

#[async_trait::async_trait]
pub trait SessionStorage {
    async fn save_session_detail(&self, session: VizierSessionDetail) -> Result<()>;

    async fn get_session_detail_by_topic(
        &self,
        agent_id: AgentId,
        chanel: VizierChannelId,
        topic: Option<TopicId>,
    ) -> Result<Option<VizierSessionDetail>>;

    async fn get_session_list(
        &self,
        agent_id: AgentId,
        chanel: VizierChannelId,
    ) -> Result<Vec<VizierSessionDetail>>;
}

#[async_trait::async_trait]
impl SessionStorage for VizierStorage {
    async fn save_session_detail(&self, session: VizierSessionDetail) -> Result<()> {
        self.0.save_session_detail(session).await
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
        chanel: VizierChannelId,
    ) -> Result<Vec<VizierSessionDetail>> {
        self.0.get_session_list(agent_id, chanel).await
    }
}
