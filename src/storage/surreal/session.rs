use anyhow::Result;

use crate::{
    schema::{AgentId, TopicId, VizierChannelId, VizierSessionDetail},
    storage::{session::SessionStorage, surreal::SurrealStorage},
};

#[async_trait::async_trait]
impl SessionStorage for SurrealStorage {
    async fn save_session_detail(&self, session: VizierSessionDetail) -> Result<()> {
        let _: Option<VizierSessionDetail> = self
            .conn
            .upsert(("session_detail", uuid::Uuid::new_v4().to_string()))
            .content(session.clone())
            .await?;

        Ok(())
    }

    async fn get_session_detail_by_topic(
        &self,
        agent_id: AgentId,
        channel: VizierChannelId,
        topic: Option<TopicId>,
    ) -> Result<Option<VizierSessionDetail>> {
        let mut response = self
            .conn
            .query("SELECT * FROM session_detail WHERE agent_id = $agent_id AND channel = $channel AND topic = $topic")
            .bind(("agent_id", agent_id))
            .bind(("channel", channel))
            .bind(("topic", topic))
            .await?;

        let detail: Option<VizierSessionDetail> = response.take(0)?;

        Ok(detail)
    }

    async fn get_session_list(
        &self,
        agent_id: AgentId,
        channel: Option<VizierChannelId>,
    ) -> Result<Vec<VizierSessionDetail>> {
        let list: Vec<VizierSessionDetail> = match channel {
            Some(ch) => {
                let mut response = self
                    .conn
                    .query("SELECT * FROM session_detail WHERE agent_id = $agent_id AND channel = $channel")
                    .bind(("agent_id", agent_id))
                    .bind(("channel", ch))
                    .await?;
                response.take(0)?
            }
            None => {
                let mut response = self
                    .conn
                    .query("SELECT * FROM session_detail WHERE agent_id = $agent_id")
                    .bind(("agent_id", agent_id))
                    .await?;
                response.take(0)?
            }
        };

        Ok(list)
    }

    async fn delete_session(
        &self,
        agent_id: AgentId,
        channel: VizierChannelId,
        topic: TopicId,
    ) -> Result<()> {
        let _: Option<VizierSessionDetail> = self
            .conn
            .query("DELETE FROM session_detail WHERE agent_id = $agent_id AND channel = $channel AND topic = $topic")
            .bind(("agent_id", agent_id))
            .bind(("channel", channel))
            .bind(("topic", topic))
            .await?
            .take(0)?;

        Ok(())
    }
}
