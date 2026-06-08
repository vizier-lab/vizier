use anyhow::Result;
use chrono::Utc;

use crate::{
    schema::{VizierSession, session_file::SessionFileRecord},
    storage::{session_file::SessionFileStorage, surreal::SurrealStorage},
};

#[async_trait::async_trait]
impl SessionFileStorage for SurrealStorage {
    async fn save_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
        mime_type: &str,
        size: u64,
        file_id: &str,
    ) -> Result<SessionFileRecord> {
        let session_slug = session.to_slug();
        let record_id = format!("{}/{}", session_slug, filename);

        let record = SessionFileRecord {
            id: record_id.clone(),
            session_slug,
            agent_id: session.0.clone(),
            filename: filename.to_string(),
            mime_type: mime_type.to_string(),
            size,
            file_id: file_id.to_string(),
            added_at: Utc::now(),
        };

        let _: Option<SessionFileRecord> = self
            .conn
            .upsert(("session_file", record_id))
            .content(record.clone())
            .await?;

        Ok(record)
    }

    async fn list_session_files(&self, session: &VizierSession) -> Result<Vec<SessionFileRecord>> {
        let session_slug = session.to_slug();
        let mut response = self
            .conn
            .query("SELECT * FROM session_file WHERE session_slug = $session_slug")
            .bind(("session_slug", session_slug))
            .await?;

        let records: Vec<SessionFileRecord> = response.take(0)?;
        Ok(records)
    }

    async fn get_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
    ) -> Result<Option<SessionFileRecord>> {
        let record_id = format!("{}/{}", session.to_slug(), filename);
        let record: Option<SessionFileRecord> =
            self.conn.select(("session_file", record_id)).await?;
        Ok(record)
    }

    async fn delete_session_file(&self, session: &VizierSession, filename: &str) -> Result<()> {
        let record_id = format!("{}/{}", session.to_slug(), filename);
        let _: Option<SessionFileRecord> = self.conn.delete(("session_file", record_id)).await?;
        Ok(())
    }
}
