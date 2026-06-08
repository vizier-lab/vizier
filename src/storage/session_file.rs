use anyhow::Result;

use crate::schema::{VizierSession, session_file::SessionFileRecord};

#[async_trait::async_trait]
pub trait SessionFileStorage {
    async fn save_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
        mime_type: &str,
        size: u64,
        file_id: &str,
    ) -> Result<SessionFileRecord>;

    async fn list_session_files(&self, session: &VizierSession) -> Result<Vec<SessionFileRecord>>;

    async fn get_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
    ) -> Result<Option<SessionFileRecord>>;

    async fn delete_session_file(&self, session: &VizierSession, filename: &str) -> Result<()>;
}
