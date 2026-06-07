use anyhow::Result;
use chrono::Utc;

use crate::{
    schema::{VizierSession, context_file::ContextFileRecord},
    storage::{context_file::ContextFileStorage, surreal::SurrealStorage},
};

#[async_trait::async_trait]
impl ContextFileStorage for SurrealStorage {
    async fn save_context_file(
        &self,
        session: &VizierSession,
        filename: &str,
        mime_type: &str,
        size: u64,
        file_id: &str,
    ) -> Result<ContextFileRecord> {
        let session_slug = session.to_slug();
        let record_id = format!("{}/{}", session_slug, filename);

        let record = ContextFileRecord {
            id: record_id.clone(),
            session_slug,
            agent_id: session.0.clone(),
            filename: filename.to_string(),
            mime_type: mime_type.to_string(),
            size,
            file_id: file_id.to_string(),
            added_at: Utc::now(),
        };

        let _: Option<ContextFileRecord> = self
            .conn
            .upsert(("context_file", record_id))
            .content(record.clone())
            .await?;

        Ok(record)
    }

    async fn list_context_files(
        &self,
        session: &VizierSession,
    ) -> Result<Vec<ContextFileRecord>> {
        let session_slug = session.to_slug();
        let mut response = self
            .conn
            .query("SELECT * FROM context_file WHERE session_slug = $session_slug")
            .bind(("session_slug", session_slug))
            .await?;

        let records: Vec<ContextFileRecord> = response.take(0)?;
        Ok(records)
    }

    async fn get_context_file(
        &self,
        session: &VizierSession,
        filename: &str,
    ) -> Result<Option<ContextFileRecord>> {
        let record_id = format!("{}/{}", session.to_slug(), filename);
        let record: Option<ContextFileRecord> = self
            .conn
            .select(("context_file", record_id))
            .await?;
        Ok(record)
    }

    async fn delete_context_file(
        &self,
        session: &VizierSession,
        filename: &str,
    ) -> Result<()> {
        let record_id = format!("{}/{}", session.to_slug(), filename);
        let _: Option<ContextFileRecord> = self
            .conn
            .delete(("context_file", record_id))
            .await?;
        Ok(())
    }
}
