use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::{
    schema::{SessionHistory, SessionHistoryContent, VizierSession},
    storage::{history::HistoryStorage, surreal::SurrealStorage},
};

#[async_trait::async_trait]
impl HistoryStorage for SurrealStorage {
    async fn save_session_history(
        &self,
        session: VizierSession,
        content: SessionHistoryContent,
    ) -> Result<()> {
        let uuid = Uuid::new_v4();
        let _: Option<SessionHistory> = self
            .conn
            .create(("session_history", uuid.clone().to_string()))
            .content(SessionHistory {
                uuid,
                session: session.clone(),
                content,
                timestamp: Utc::now(),
            })
            .await?;

        Ok(())
    }

    async fn list_session_history(&self, session: VizierSession) -> Result<Vec<SessionHistory>> {
        let mut response = self
            .conn
            .query(format!(
                "SELECT * FROM session_history WHERE vizier_session == $session ORDER BY timestamp ASC",
            ))
            .bind(("session", session.clone()))
            .await?;

        let list: Vec<SessionHistory> = response.take(0)?;

        Ok(list)
    }
}
