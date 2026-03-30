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
                uid: uuid.to_string(),
                vizier_session: session.clone(),
                content,
                timestamp: Utc::now(),
            })
            .await?;

        Ok(())
    }

    async fn list_session_history(
        &self,
        session: VizierSession,
        before: Option<chrono::DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<SessionHistory>> {
        let query = if let Some(before_dt) = before {
            if let Some(limit_val) = limit {
                format!(
                    "SELECT * FROM session_history WHERE vizier_session == $vizier_session AND timestamp < {} ORDER BY timestamp DESC LIMIT {}",
                    before_dt.timestamp_millis(),
                    limit_val
                )
            } else {
                format!(
                    "SELECT * FROM session_history WHERE vizier_session == $vizier_session AND timestamp < {} ORDER BY timestamp DESC",
                    before_dt.timestamp_millis()
                )
            }
        } else if let Some(limit_val) = limit {
            format!(
                "SELECT * FROM session_history WHERE vizier_session == $vizier_session ORDER BY timestamp DESC LIMIT {}",
                limit_val
            )
        } else {
            "SELECT * FROM session_history WHERE vizier_session == $vizier_session ORDER BY timestamp DESC"
                .to_string()
        };

        let mut response = self
            .conn
            .query(query)
            .bind(("vizier_session", session.clone()))
            .await?;

        let mut list: Vec<SessionHistory> = response.take(0)?;

        // Sort back to ascending order (oldest first) for the final result
        list.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(list)
    }
}
