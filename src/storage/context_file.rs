use anyhow::Result;

use crate::schema::{VizierSession, context_file::ContextFileRecord};

#[async_trait::async_trait]
pub trait ContextFileStorage {
    async fn save_context_file(
        &self,
        session: &VizierSession,
        filename: &str,
        mime_type: &str,
        size: u64,
        file_id: &str,
    ) -> Result<ContextFileRecord>;

    async fn list_context_files(
        &self,
        session: &VizierSession,
    ) -> Result<Vec<ContextFileRecord>>;

    async fn get_context_file(
        &self,
        session: &VizierSession,
        filename: &str,
    ) -> Result<Option<ContextFileRecord>>;

    async fn delete_context_file(
        &self,
        session: &VizierSession,
        filename: &str,
    ) -> Result<()>;
}
