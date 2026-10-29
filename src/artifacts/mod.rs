//! The artifact service (`specs/035-add-artifacts`): promote, replace, list, detail, delete,
//! content and the one resolver that turns an artifact identity into bytes.
//!
//! Metadata, references and events go through [`crate::storage::artifact::ArtifactStorage`];
//! bytes live in [`paths`]. One mutation lock serialises writes.

pub mod access;
mod paths;

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::error::VizierError;
use crate::file_manager::FileManager;
use crate::schema::{
    Artifact, ArtifactEvent, ArtifactEventAction, ArtifactKind, ArtifactOrigin, AttachmentResolver,
    Requester, VizierSession,
};
use crate::storage::VizierStorage;
use crate::storage::artifact::ArtifactStorage;
use crate::storage::memory::MemoryStorage;
use crate::storage::session_file::SessionFileStorage;

pub use paths::ArtifactPaths;

/// A promotion/replacement source: the session file name and the conversation it belongs to.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub session: VizierSession,
    pub filename: String,
}

fn origin_of(session: &VizierSession, source_filename: Option<String>) -> ArtifactOrigin {
    ArtifactOrigin {
        channel: Some(session.1.to_slug()),
        topic: session.2.clone(),
        source_filename,
    }
}

fn revision_origin(actor: &Requester) -> crate::schema::RevisionOrigin {
    use crate::schema::{RevisionActor, RevisionOrigin, RevisionTrigger};
    match actor {
        Requester::User(user) => RevisionOrigin {
            actor: RevisionActor::User {
                user_id: user.clone(),
                username: user.clone(),
            },
            trigger: RevisionTrigger::Api,
            dream_cycle: None,
        },
        Requester::Agent(agent) => RevisionOrigin {
            actor: RevisionActor::Agent,
            trigger: RevisionTrigger::Conversation,
            dream_cycle: None,
        },
    }
}

/// What a promotion/replacement produced, for the HTTP view and tools.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ArtifactWrite {
    pub artifact: Artifact,
    pub available: bool,
}

pub struct Artifacts {
    workspace: PathBuf,
    storage: Arc<VizierStorage>,
    paths: ArtifactPaths,
    file_manager: FileManager,
    /// Serialises promote/replace/delete. Writes are rare; one lock avoids per-artifact
    /// bookkeeping (contracts/artifact-storage.md §7).
    mutations: tokio::sync::Mutex<()>,
}

impl Artifacts {
    pub fn new(
        workspace: impl Into<PathBuf>,
        storage: Arc<VizierStorage>,
        file_manager: FileManager,
    ) -> Self {
        let workspace = workspace.into();
        Self {
            paths: ArtifactPaths::new(workspace.clone()),
            workspace,
            storage,
            file_manager,
            mutations: tokio::sync::Mutex::new(()),
        }
    }

    pub fn storage(&self) -> &Arc<VizierStorage> {
        &self.storage
    }

    pub fn paths(&self) -> &ArtifactPaths {
        &self.paths
    }

    pub fn file_manager(&self) -> &FileManager {
        &self.file_manager
    }

    pub fn workspace(&self) -> &PathBuf {
        &self.workspace
    }

    /// The path of an artifact's current content, when both the row and the file exist.
    pub fn path_for(&self, agent_id: &str, artifact_id: &str) -> crate::Result<Option<PathBuf>> {
        let file = self
            .storage
            .artifact_content_file(agent_id, artifact_id)
            .map_err(|e| VizierError(e.to_string()))?;
        Ok(file.and_then(|file| {
            self.paths
                .exists(agent_id, artifact_id, &file)
                .then(|| self.paths.content_path(agent_id, artifact_id, &file))
        }))
    }

    /// The artifact's current bytes.
    pub async fn read_content(&self, agent_id: &str, artifact_id: &str) -> crate::Result<Vec<u8>> {
        let file = self
            .storage
            .artifact_content_file(agent_id, artifact_id)
            .map_err(|e| VizierError(e.to_string()))?
            .ok_or_else(|| VizierError("artifact not found".into()))?;
        self.paths
            .read_current(agent_id, artifact_id, &file)
            .await
            .map_err(|e| VizierError(e.to_string()))
    }

    /// Remove an agent's whole content directory after a successful storage delete.
    pub async fn delete_agent_files(&self, agent_id: &str) {
        if let Err(e) = self.paths.delete_agent(agent_id).await {
            tracing::warn!(agent_id, "failed to remove agent artifacts: {e}");
        }
    }

    pub(crate) fn promoted_event(
        &self,
        artifact: &Artifact,
        actor: Requester,
        origin: &ArtifactOrigin,
    ) -> ArtifactEvent {
        ArtifactEvent {
            id: Uuid::new_v4().to_string(),
            agent_id: artifact.agent_id.clone(),
            artifact_id: artifact.id.clone(),
            artifact_name: artifact.name.clone(),
            action: ArtifactEventAction::Promoted,
            actor,
            source_channel: origin.channel.clone(),
            source_topic: origin.topic.clone(),
            source_filename: origin.source_filename.clone(),
            created_at: Utc::now(),
        }
    }

    /// The kind of a file, from its name and bytes-derived MIME guess.
    pub(crate) fn kind_for(name: &str, mime: &str) -> ArtifactKind {
        ArtifactKind::from_mime(mime, name)
    }

    /// A session's origin metadata.
    /// Read the source session file's bytes and its MIME guess.
    async fn read_source(&self, source: &SourceFile) -> crate::Result<(Vec<u8>, String, String)> {
        let record = self
            .storage
            .get_session_file(&source.session, &source.filename)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .ok_or_else(|| {
                VizierError(format!(
                    "session file '{}' not found in this conversation",
                    source.filename
                ))
            })?;
        let (_, bytes) =
            crate::file_manager::read_upload(&self.workspace_string(), &record.file_id)
                .await
                .map_err(|e| VizierError(format!("failed to read '{}': {e}", source.filename)))?;
        if bytes.is_empty() {
            return Err(VizierError(format!(
                "session file '{}' is empty",
                source.filename
            )));
        }
        let mime = if record.mime_type.is_empty() {
            mime_guess::from_path(&record.filename)
                .first_or_octet_stream()
                .to_string()
        } else {
            record.mime_type
        };
        Ok((bytes, mime, record.filename))
    }

    fn workspace_string(&self) -> String {
        self.workspace.to_string_lossy().to_string()
    }

    /// Promote a session file into a new artifact. Copies the bytes; the source stays where it
    /// is and is not marked.
    pub async fn promote(
        &self,
        agent_id: &str,
        actor: Requester,
        source: &SourceFile,
        name: Option<&str>,
    ) -> crate::Result<Artifact> {
        let _guard = self.mutations.lock().await;

        let name = crate::schema::validate_artifact_name(name.unwrap_or(&source.filename))?;
        if self
            .storage
            .get_artifact_by_name(agent_id, &name)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .is_some()
        {
            return Err(VizierError(format!(
                "an artifact named '{name}' already exists on this agent; replace it instead of promoting it again"
            )));
        }

        let (bytes, mime, source_filename) = self.read_source(source).await?;
        let size = bytes.len() as u64;
        let artifact_id = Uuid::new_v4().to_string();
        let kind = ArtifactKind::from_mime(&mime, &name);
        let file = self
            .paths
            .write_version(agent_id, &artifact_id, 1, &bytes)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        let now = Utc::now();
        let origin = origin_of(&source.session, Some(source_filename));
        let artifact = Artifact {
            id: artifact_id.clone(),
            agent_id: agent_id.to_string(),
            name: name.clone(),
            kind,
            mime_type: mime,
            size,
            version: 1,
            content_file: file,
            origin_channel: origin.channel.clone(),
            origin_topic: origin.topic.clone(),
            source_filename: origin.source_filename.clone(),
            created_by: actor.clone(),
            created_at: now,
            updated_at: now,
        };
        let event = self.promoted_event(&artifact, actor, &origin);
        if let Err(e) = self.storage.create_artifact(&artifact, &event).await {
            let _ = self.paths.delete_artifact(agent_id, &artifact_id).await;
            return Err(VizierError(e.to_string()));
        }
        Ok(artifact)
    }

    /// Replace an artifact's content from a session file, keeping its identity and name. The
    /// new version file is written first; the row switches only after it is on disk.
    pub async fn replace(
        &self,
        agent_id: &str,
        actor: Requester,
        name: &str,
        source: &SourceFile,
    ) -> crate::Result<Artifact> {
        let _guard = self.mutations.lock().await;

        let artifact = self
            .storage
            .get_artifact_by_name(agent_id, name)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .ok_or_else(|| VizierError(format!("no artifact named '{name}'")))?;
        let (bytes, mime, source_filename) = self.read_source(source).await?;
        let size = bytes.len() as u64;
        let next_version = artifact.version + 1;
        let file_name = format!("content.v{next_version}");
        let target = self.paths.content_path(agent_id, &artifact.id, &file_name);
        // A leftover from a failed earlier replacement (crash between the write and the row
        // switch) is removed rather than treated as a live version.
        if target.exists() && file_name != artifact.content_file {
            let _ = tokio::fs::remove_file(&target).await;
        }
        let file = self
            .paths
            .write_version(agent_id, &artifact.id, next_version, &bytes)
            .await
            .map_err(|e| VizierError(e.to_string()))?;
        let now = Utc::now();
        let kind = ArtifactKind::from_mime(&mime, name);
        let origin = origin_of(&source.session, Some(source_filename.clone()));
        let event = ArtifactEvent {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            artifact_id: artifact.id.clone(),
            artifact_name: artifact.name.clone(),
            action: ArtifactEventAction::Replaced,
            actor,
            source_channel: origin.channel.clone(),
            source_topic: origin.topic.clone(),
            source_filename: Some(source_filename.clone()),
            created_at: now,
        };

        match self
            .storage
            .replace_artifact(
                agent_id,
                &artifact.id,
                artifact.version,
                &file,
                size,
                &mime,
                kind,
                &source_filename,
                now,
                &event,
            )
            .await
        {
            Ok(updated) => {
                self.remove_other_versions(agent_id, &artifact.id, &updated.content_file)
                    .await;
                Ok(updated)
            }
            Err(e) => {
                let _ =
                    tokio::fs::remove_file(self.paths.content_path(agent_id, &artifact.id, &file))
                        .await;
                Err(VizierError(e.to_string()))
            }
        }
    }

    /// Remove version files other than the current one. Best effort; a failure is retried on
    /// the next replacement or deletion.
    async fn remove_other_versions(&self, agent_id: &str, artifact_id: &str, current: &str) {
        let dir = self.paths.artifact_dir(agent_id, artifact_id);
        let Ok(mut entries) = tokio::fs::read_dir(&dir).await else {
            return;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("content.v") && name != current {
                if let Err(e) = tokio::fs::remove_file(entry.path()).await {
                    tracing::warn!(
                        agent_id,
                        artifact_id,
                        file = %name,
                        "failed to remove an old artifact version: {e}"
                    );
                }
            }
        }
    }

    /// Delete an artifact: row, references and content, with a `deleted` event that survives.
    /// Memory links are removed with a revision; message references become unavailable.
    pub async fn delete(
        &self,
        agent_id: &str,
        artifact_id: &str,
        actor: Requester,
    ) -> crate::Result<crate::schema::ArtifactDeleteResult> {
        let _guard = self.mutations.lock().await;
        let artifact = self
            .storage
            .get_artifact(agent_id, artifact_id)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .ok_or_else(|| VizierError("artifact not found".into()))?;
        let references = self
            .storage
            .list_artifact_references(agent_id, artifact_id)
            .await
            .map_err(|e| VizierError(e.to_string()))?;
        let memory_refs = references.iter().filter(|r| r.kind == "memory").count();
        let event = ArtifactEvent {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            artifact_id: artifact_id.to_string(),
            artifact_name: artifact.name.clone(),
            action: ArtifactEventAction::Deleted,
            actor: actor.clone(),
            source_channel: artifact.origin_channel.clone(),
            source_topic: artifact.origin_topic.clone(),
            source_filename: artifact.source_filename.clone(),
            created_at: Utc::now(),
        };
        // The row goes first, so a concurrent memory attach cannot validate it again.
        self.storage
            .delete_artifact(agent_id, artifact_id, &event)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        let origin = revision_origin(&actor);
        let memory_links_removed = match self
            .storage
            .detach_artifact_from_memories(agent_id.to_string(), artifact_id.to_string(), &origin)
            .await
        {
            Ok(count) => count,
            Err(e) => {
                tracing::warn!(
                    agent_id,
                    artifact_id,
                    "failed to detach a deleted artifact from memories; links read as deleted: {e}"
                );
                memory_refs
            }
        };
        if let Err(e) = self.paths.delete_artifact(agent_id, artifact_id).await {
            tracing::warn!(
                agent_id,
                artifact_id,
                "failed to remove artifact content: {e}"
            );
        }
        Ok(crate::schema::ArtifactDeleteResult {
            removed: true,
            references: references.len(),
            memory_links_removed,
        })
    }
}

/// One artifact with everything its detail view needs.
#[derive(Debug, Clone)]
pub struct ArtifactDetail {
    pub artifact: Artifact,
    pub available: bool,
    pub references: Vec<crate::schema::ArtifactReference>,
    pub events: Vec<ArtifactEvent>,
}

impl Artifacts {
    pub async fn list(
        &self,
        filters: crate::schema::ArtifactFilters,
    ) -> crate::Result<crate::schema::ArtifactPage> {
        self.storage
            .list_artifacts(&filters)
            .await
            .map_err(|e| VizierError(e.to_string()))
    }

    pub fn is_available(&self, agent_id: &str, artifact: &Artifact) -> bool {
        self.paths
            .exists(agent_id, &artifact.id, &artifact.content_file)
    }

    /// The artifact with its references and full change history. Availability is checked
    /// against the current content file, never stored.
    pub async fn detail(
        &self,
        agent_id: &str,
        artifact_id: &str,
    ) -> crate::Result<Option<ArtifactDetail>> {
        let Some(artifact) = self
            .storage
            .get_artifact(agent_id, artifact_id)
            .await
            .map_err(|e| VizierError(e.to_string()))?
        else {
            return Ok(None);
        };
        let references = self
            .storage
            .list_artifact_references(agent_id, artifact_id)
            .await
            .map_err(|e| VizierError(e.to_string()))?;
        let events = self
            .storage
            .list_artifact_events(agent_id, None, 50)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .into_iter()
            .filter(|event| event.artifact_id == artifact_id)
            .collect();
        Ok(Some(ArtifactDetail {
            available: self.is_available(agent_id, &artifact),
            artifact,
            references,
            events,
        }))
    }

    /// One page of the agent's change log, newest first.
    pub async fn activity(
        &self,
        agent_id: &str,
        cursor: Option<crate::schema::ArtifactCursor>,
        limit: usize,
    ) -> crate::Result<Vec<ArtifactEvent>> {
        self.storage
            .list_artifact_events(agent_id, cursor, limit)
            .await
            .map_err(|e| VizierError(e.to_string()))
    }

    pub async fn stats(&self, agent_id: &str) -> crate::Result<crate::schema::ArtifactStats> {
        self.storage
            .artifact_stats(agent_id)
            .await
            .map_err(|e| VizierError(e.to_string()))
    }
}

impl AttachmentResolver for Artifacts {
    fn artifact_path(&self, agent_id: &str, artifact_id: &str) -> anyhow::Result<Option<PathBuf>> {
        let file = self.storage.artifact_content_file(agent_id, artifact_id)?;
        Ok(file.and_then(|file| {
            self.paths
                .exists(agent_id, artifact_id, &file)
                .then(|| self.paths.content_path(agent_id, artifact_id, &file))
        }))
    }
}

/// The safe, server-derived session-file name used when an artifact is copied into a
/// conversation (`artifact-<id><ext>`); the artifact's display name never becomes a path.
pub fn session_filename(artifact_id: &str, name: &str) -> String {
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric()));
    match ext {
        Some(ext) => format!("artifact-{artifact_id}.{ext}"),
        None => format!("artifact-{artifact_id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::document::LocalDocumentStore;
    use parking_lot::Mutex;
    use std::sync::Arc;

    fn service() -> (Artifacts, Arc<VizierStorage>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_artifact_schema(&conn).unwrap();
        let storage = Arc::new(VizierStorage::new(
            crate::storage::sqlite::SqliteStorage::new(
                Arc::new(Mutex::new(conn)),
                Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
            ),
        ));
        let fm = FileManager::new(dir.path().to_string_lossy().to_string());
        (
            Artifacts::new(dir.path(), storage.clone(), fm),
            storage,
            dir,
        )
    }

    #[tokio::test]
    async fn resolution_needs_both_the_row_and_the_file() {
        let (artifacts, storage, _d) = service();
        let a = Artifact {
            id: "id".into(),
            agent_id: "a".into(),
            name: "x.txt".into(),
            kind: ArtifactKind::Text,
            mime_type: "text/plain".into(),
            size: 1,
            version: 1,
            content_file: "content.v1".into(),
            origin_channel: None,
            origin_topic: None,
            source_filename: None,
            created_by: Requester::Agent("a".into()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        storage
            .create_artifact(
                &a,
                &artifacts.promoted_event(&a, Requester::Agent("a".into()), &Default::default()),
            )
            .await
            .unwrap();
        // Row exists, file does not.
        assert!(artifacts.path_for("a", "id").unwrap().is_none());
        let file = artifacts
            .paths()
            .write_version("a", "id", 1, b"hi")
            .await
            .unwrap();
        assert_eq!(file, "content.v1");
        assert!(artifacts.path_for("a", "id").unwrap().is_some());
        assert_eq!(artifacts.read_content("a", "id").await.unwrap(), b"hi");
        // Deleting the row makes resolution unavailable even while the file exists.
        artifacts
            .storage()
            .delete_artifact(
                "a",
                "id",
                &ArtifactEvent {
                    id: "e".into(),
                    agent_id: "a".into(),
                    artifact_id: "id".into(),
                    artifact_name: "x.txt".into(),
                    action: ArtifactEventAction::Deleted,
                    actor: Requester::Agent("a".into()),
                    source_channel: None,
                    source_topic: None,
                    source_filename: None,
                    created_at: Utc::now(),
                },
            )
            .await
            .unwrap();
        assert!(artifacts.path_for("a", "id").unwrap().is_none());
    }
}
