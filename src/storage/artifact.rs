//! Artifact metadata storage (`specs/035-add-artifacts`, contracts/artifact-storage.md).
//!
//! Metadata, references and events are in SQLite; the bytes are on disk. Every mutation takes
//! the event it records so the row change and its event share one transaction.

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::schema::{
    Artifact, ArtifactCursor, ArtifactEvent, ArtifactFilters, ArtifactKind, ArtifactPage,
    ArtifactReference, ArtifactStats,
};

#[async_trait::async_trait]
#[allow(clippy::too_many_arguments)]
pub trait ArtifactStorage {
    async fn create_artifact(&self, artifact: &Artifact, event: &ArtifactEvent) -> Result<()>;

    async fn get_artifact(&self, agent_id: &str, id: &str) -> Result<Option<Artifact>>;

    async fn get_artifact_by_name(&self, agent_id: &str, name: &str) -> Result<Option<Artifact>>;

    async fn list_artifacts(&self, filters: &ArtifactFilters) -> Result<ArtifactPage>;

    async fn artifact_stats(&self, agent_id: &str) -> Result<ArtifactStats>;

    async fn replace_artifact(
        &self,
        agent_id: &str,
        id: &str,
        expected_version: u32,
        content_file: &str,
        size: u64,
        mime_type: &str,
        kind: ArtifactKind,
        source_filename: &str,
        updated_at: DateTime<Utc>,
        event: &ArtifactEvent,
    ) -> Result<Artifact>;

    async fn delete_artifact(&self, agent_id: &str, id: &str, event: &ArtifactEvent) -> Result<()>;

    async fn list_artifact_events(
        &self,
        agent_id: &str,
        before: Option<ArtifactCursor>,
        limit: usize,
    ) -> Result<Vec<ArtifactEvent>>;

    async fn list_artifact_references(
        &self,
        agent_id: &str,
        artifact_id: &str,
    ) -> Result<Vec<ArtifactReference>>;

    async fn count_artifact_references(&self, agent_id: &str, artifact_id: &str) -> Result<usize>;

    async fn upsert_artifact_reference(&self, reference: &ArtifactReference) -> Result<()>;

    async fn delete_artifact_references(&self, agent_id: &str, artifact_id: &str) -> Result<()>;

    async fn delete_message_references_for_session(
        &self,
        agent_id: &str,
        session_slug: &str,
    ) -> Result<()>;

    async fn delete_memory_references(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
    ) -> Result<()>;

    async fn delete_agent_artifact_data(&self, agent_id: &str) -> Result<()>;

    /// Synchronous because model-part conversion (`to_part`) and `FileManager::resolve` are
    /// synchronous. One indexed read under the existing connection lock.
    fn artifact_content_file(&self, agent_id: &str, id: &str) -> Result<Option<String>>;
}

#[async_trait::async_trait]
impl ArtifactStorage for crate::storage::VizierStorage {
    async fn create_artifact(&self, artifact: &Artifact, event: &ArtifactEvent) -> Result<()> {
        self.0.create_artifact(artifact, event).await
    }

    async fn get_artifact(&self, agent_id: &str, id: &str) -> Result<Option<Artifact>> {
        self.0.get_artifact(agent_id, id).await
    }

    async fn get_artifact_by_name(&self, agent_id: &str, name: &str) -> Result<Option<Artifact>> {
        self.0.get_artifact_by_name(agent_id, name).await
    }

    async fn list_artifacts(&self, filters: &ArtifactFilters) -> Result<ArtifactPage> {
        self.0.list_artifacts(filters).await
    }

    async fn artifact_stats(&self, agent_id: &str) -> Result<ArtifactStats> {
        self.0.artifact_stats(agent_id).await
    }

    async fn replace_artifact(
        &self,
        agent_id: &str,
        id: &str,
        expected_version: u32,
        content_file: &str,
        size: u64,
        mime_type: &str,
        kind: ArtifactKind,
        source_filename: &str,
        updated_at: DateTime<Utc>,
        event: &ArtifactEvent,
    ) -> Result<Artifact> {
        self.0
            .replace_artifact(
                agent_id,
                id,
                expected_version,
                content_file,
                size,
                mime_type,
                kind,
                source_filename,
                updated_at,
                event,
            )
            .await
    }

    async fn delete_artifact(&self, agent_id: &str, id: &str, event: &ArtifactEvent) -> Result<()> {
        self.0.delete_artifact(agent_id, id, event).await
    }

    async fn list_artifact_events(
        &self,
        agent_id: &str,
        before: Option<ArtifactCursor>,
        limit: usize,
    ) -> Result<Vec<ArtifactEvent>> {
        self.0.list_artifact_events(agent_id, before, limit).await
    }

    async fn list_artifact_references(
        &self,
        agent_id: &str,
        artifact_id: &str,
    ) -> Result<Vec<ArtifactReference>> {
        self.0.list_artifact_references(agent_id, artifact_id).await
    }

    async fn count_artifact_references(&self, agent_id: &str, artifact_id: &str) -> Result<usize> {
        self.0
            .count_artifact_references(agent_id, artifact_id)
            .await
    }

    async fn upsert_artifact_reference(&self, reference: &ArtifactReference) -> Result<()> {
        self.0.upsert_artifact_reference(reference).await
    }

    async fn delete_artifact_references(&self, agent_id: &str, artifact_id: &str) -> Result<()> {
        self.0
            .delete_artifact_references(agent_id, artifact_id)
            .await
    }

    async fn delete_message_references_for_session(
        &self,
        agent_id: &str,
        session_slug: &str,
    ) -> Result<()> {
        self.0
            .delete_message_references_for_session(agent_id, session_slug)
            .await
    }

    async fn delete_memory_references(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
    ) -> Result<()> {
        self.0
            .delete_memory_references(agent_id, bundle, path)
            .await
    }

    async fn delete_agent_artifact_data(&self, agent_id: &str) -> Result<()> {
        self.0.delete_agent_artifact_data(agent_id).await
    }

    fn artifact_content_file(&self, agent_id: &str, id: &str) -> Result<Option<String>> {
        self.0.artifact_content_file(agent_id, id)
    }
}
