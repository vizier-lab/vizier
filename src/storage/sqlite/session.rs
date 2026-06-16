use anyhow::Result;

use crate::{
    schema::{AgentId, TopicId, VizierChannelId, VizierSessionDetail},
    storage::{session::SessionStorage, sqlite::SqliteStorage},
};

fn session_detail_id(agent_id: &str, channel: &VizierChannelId, topic: &Option<TopicId>) -> String {
    format!(
        "{}#{}#{}",
        agent_id,
        channel.to_slug(),
        topic.clone().unwrap_or("DEFAULT".into())
    )
}

#[async_trait::async_trait]
impl SessionStorage for SqliteStorage {
    async fn save_session_detail(&self, session: VizierSessionDetail) -> Result<()> {
        let id = session_detail_id(&session.agent_id, &session.channel, &session.topic);
        let data = serde_json::to_string(&session)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO session_detail (id, agent_id, channel, topic, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, session.agent_id, session.channel.to_slug(), session.topic, data],
        )?;
        Ok(())
    }

    async fn update_session_detail(&self, session: VizierSessionDetail) -> Result<()> {
        let data = serde_json::to_string(&session)?;
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE session_detail SET data = ?1 WHERE agent_id = ?2 AND channel = ?3 AND topic = ?4",
            rusqlite::params![data, session.agent_id, session.channel.to_slug(), session.topic],
        )?;
        Ok(())
    }

    async fn update_thinking_state(
        &self,
        agent_id: AgentId,
        channel: VizierChannelId,
        topic: Option<TopicId>,
        is_thinking: bool,
    ) -> Result<()> {
        let conn = self.conn.lock();
        // Read existing
        let data: String = {
            let mut stmt = conn.prepare(
                "SELECT data FROM session_detail WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3",
            )?;
            let mut rows = stmt.query_map(
                rusqlite::params![agent_id, channel.to_slug(), topic],
                |row| {
                    let data: String = row.get(0)?;
                    Ok(data)
                },
            )?;
            match rows.next() {
                Some(Ok(d)) => d,
                _ => return Ok(()), // Not found, nothing to update
            }
        };

        let mut detail: VizierSessionDetail = serde_json::from_str(&data)?;
        detail.is_thinking = is_thinking;

        let new_data = serde_json::to_string(&detail)?;
        conn.execute(
            "UPDATE session_detail SET data = ?1 WHERE agent_id = ?2 AND channel = ?3 AND topic = ?4",
            rusqlite::params![new_data, agent_id, channel.to_slug(), topic],
        )?;
        Ok(())
    }

    async fn get_session_detail_by_topic(
        &self,
        agent_id: AgentId,
        channel: VizierChannelId,
        topic: Option<TopicId>,
    ) -> Result<Option<VizierSessionDetail>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT data FROM session_detail WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3",
        )?;
        let mut rows = stmt.query_map(
            rusqlite::params![agent_id, channel.to_slug(), topic],
            |row| {
                let data: String = row.get(0)?;
                Ok(data)
            },
        )?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn get_session_list(
        &self,
        agent_id: AgentId,
        channel: Option<VizierChannelId>,
    ) -> Result<Vec<VizierSessionDetail>> {
        let conn = self.conn.lock();
        let (sql, params): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match &channel {
            Some(ch) => (
                "SELECT data FROM session_detail WHERE agent_id = ?1 AND channel = ?2",
                vec![
                    Box::new(agent_id.clone()) as Box<dyn rusqlite::types::ToSql>,
                    Box::new(ch.to_slug()),
                ],
            ),
            None => (
                "SELECT data FROM session_detail WHERE agent_id = ?1",
                vec![Box::new(agent_id.clone()) as Box<dyn rusqlite::types::ToSql>],
            ),
        };

        let mut stmt = conn.prepare(sql)?;
        let details = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<VizierSessionDetail>(&data).ok())
            .collect();
        Ok(details)
    }

    async fn delete_session(
        &self,
        agent_id: AgentId,
        channel: VizierChannelId,
        topic: TopicId,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM session_detail WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3",
            rusqlite::params![agent_id, channel.to_slug(), topic],
        )?;
        Ok(())
    }
}
