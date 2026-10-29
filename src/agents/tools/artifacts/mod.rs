//! Artifact tools (`specs/035-add-artifacts`, contracts/agent-tools-and-memory.md §1).
//!
//! `promote_artifact`, `update_artifact`, `list_artifacts` and `load_artifact_as_session_file`
//! live in the existing **Files** category. A dream may use promote/list (and read its source
//! sessions' files) but never update or write a session file.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::agents::tools::{ToolCategory, ToolContext, VizierTool};
use crate::error::VizierError;
use crate::file_manager::FileManager;
use crate::schema::{Artifact, ArtifactKind, Requester, VizierChannelId, VizierSession};
use crate::storage::VizierStorage;
use crate::storage::artifact::ArtifactStorage;
use crate::storage::session_file::SessionFileStorage;

/// Resolve the conversation a tool reads a session file from: the current one in an ordinary
/// turn, or one of `ToolContext.dream_sources` in a dream (`specs/035-add-artifacts` R16).
pub fn resolve_source_session(
    ctx: &ToolContext,
    source_session: Option<&str>,
) -> Result<VizierSession, VizierError> {
    if matches!(ctx.session.1, VizierChannelId::Dream(..)) {
        if ctx.dream_sources.is_empty() {
            return Err(VizierError(
                "this dream cycle has no source conversations to read files from".into(),
            ));
        }
        match source_session {
            Some(slug) => ctx
                .dream_sources
                .iter()
                .find(|session| {
                    session.to_slug() == slug || session.1.to_slug() == slug
                })
                .cloned()
                .ok_or_else(|| {
                    VizierError(format!(
                        "session '{slug}' is not one of this dream cycle's source conversations; it may only read its own sources"
                    ))
                }),
            None if ctx.dream_sources.len() == 1 => Ok(ctx.dream_sources[0].clone()),
            None => Err(VizierError(
                "this dream cycle has several source conversations; name one with source_session"
                    .into(),
            )),
        }
    } else {
        match source_session {
            None => Ok(ctx.session.clone()),
            Some(slug) if slug == ctx.session.to_slug() || slug == ctx.session.1.to_slug() => {
                Ok(ctx.session.clone())
            }
            Some(slug) => Err(VizierError(format!(
                "session '{slug}' is not this conversation; only its own session files can be used"
            ))),
        }
    }
}

fn source_file(
    ctx: &ToolContext,
    filename: String,
    source_session: Option<&str>,
) -> Result<crate::artifacts::SourceFile, VizierError> {
    Ok(crate::artifacts::SourceFile {
        session: resolve_source_session(ctx, source_session)?,
        filename,
    })
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PromoteArtifactArgs {
    #[schemars(description = "the session file to copy into a permanent artifact")]
    pub filename: String,
    #[schemars(
        description = "the artifact's display name; defaults to the source filename. Names are unique per agent and case-sensitive."
    )]
    #[serde(default)]
    pub name: Option<String>,
    #[schemars(
        description = "the conversation to read the file from. Omit for the current conversation; in a dream, omit only when the cycle has one source, otherwise use the source slug."
    )]
    #[serde(default)]
    pub source_session: Option<String>,
}

pub struct PromoteArtifact {
    pub artifacts: Arc<crate::artifacts::Artifacts>,
}

#[async_trait::async_trait]
impl VizierTool for PromoteArtifact {
    type Input = PromoteArtifactArgs;
    type Output = Artifact;

    fn name() -> String {
        "promote_artifact".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Files)
    }

    fn description(&self) -> String {
        "Copy a session file into a permanent artifact that outlives this conversation. The source \
         file stays where it is and is not marked. Artifacts are unique per agent by name; use \
         update_artifact to change one's content. Only the calling agent's artifacts are visible."
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let source = source_file(ctx, args.filename, args.source_session.as_deref())?;
        self.artifacts
            .promote(
                &ctx.session.0,
                Requester::Agent(ctx.session.0.clone()),
                &source,
                args.name.as_deref(),
            )
            .await
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UpdateArtifactArgs {
    #[schemars(description = "the existing artifact's display name")]
    pub name: String,
    #[schemars(description = "the session file holding the new content")]
    pub filename: String,
    #[schemars(
        description = "the conversation to read the file from. Omit for the current conversation."
    )]
    #[serde(default)]
    pub source_session: Option<String>,
}

pub struct UpdateArtifact {
    pub artifacts: Arc<crate::artifacts::Artifacts>,
}

#[async_trait::async_trait]
impl VizierTool for UpdateArtifact {
    type Input = UpdateArtifactArgs;
    type Output = Artifact;

    fn name() -> String {
        "update_artifact".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Files)
    }

    fn description(&self) -> String {
        "Replace an existing artifact's content from a session file, keeping its id and name and \
         incrementing its version. A failed copy leaves the old content untouched."
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let source = source_file(ctx, args.filename, args.source_session.as_deref())?;
        self.artifacts
            .replace(
                &ctx.session.0,
                Requester::Agent(ctx.session.0.clone()),
                &args.name,
                &source,
            )
            .await
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListArtifactsArgs {
    #[schemars(
        description = "optional kind filter: image, pdf, text, markdown, csv, audio, video, other"
    )]
    #[serde(default)]
    pub kind: Option<ArtifactKind>,
}

pub struct ListArtifacts {
    pub artifacts: Arc<crate::artifacts::Artifacts>,
}

#[async_trait::async_trait]
impl VizierTool for ListArtifacts {
    type Input = ListArtifactsArgs;
    type Output = Vec<Artifact>;

    fn name() -> String {
        "list_artifacts".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Files)
    }

    fn description(&self) -> String {
        "List this agent's permanent artifacts, newest change first, optionally filtered by kind. \
         Each carries its name, kind, MIME type, size, version and last change."
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let mut filters = crate::schema::ArtifactFilters {
            agent_id: ctx.session.0.clone(),
            kind: args.kind,
            limit: 100,
            ..Default::default()
        };
        let mut items = Vec::new();
        loop {
            let page = self
                .artifacts
                .storage()
                .list_artifacts(&filters)
                .await
                .map_err(|e| VizierError(e.to_string()))?;
            items.extend(page.items);
            match page
                .next
                .and_then(|c| crate::schema::ArtifactCursor::decode(&c))
            {
                Some(cursor) => filters.cursor = Some(cursor),
                None => break,
            }
        }
        Ok(items)
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LoadArtifactArgs {
    #[schemars(description = "the artifact's display name")]
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LoadedArtifactFile {
    pub session_filename: String,
    pub artifact_name: String,
    pub mime_type: String,
    pub size: u64,
}

pub struct LoadArtifactAsSessionFile {
    pub artifacts: Arc<crate::artifacts::Artifacts>,
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

#[async_trait::async_trait]
impl VizierTool for LoadArtifactAsSessionFile {
    type Input = LoadArtifactArgs;
    type Output = LoadedArtifactFile;

    fn name() -> String {
        "load_artifact_as_session_file".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Files)
    }

    fn description(&self) -> String {
        "Copy one of this agent's artifacts into the current conversation as a session file, under \
         a safe server-derived name, so read_document_file/read_image_file can open it. This is a \
         transient copy: the artifact itself is unchanged."
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let artifact = self
            .storage
            .get_artifact_by_name(&ctx.session.0, &args.name)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .ok_or_else(|| VizierError(format!("no artifact named '{}'", args.name)))?;
        let bytes = self
            .artifacts
            .read_content(&ctx.session.0, &artifact.id)
            .await?;
        let safe = crate::artifacts::session_filename(&artifact.id, &artifact.name);
        let record = self
            .file_manager
            .upload(&safe, bytes)
            .await
            .map_err(|e| VizierError(e.to_string()))?;
        self.storage
            .save_session_file(
                &ctx.session,
                &safe,
                &artifact.mime_type,
                record.size,
                &record.id,
            )
            .await
            .map_err(|e| VizierError(e.to_string()))?;
        Ok(LoadedArtifactFile {
            session_filename: safe,
            artifact_name: artifact.name,
            mime_type: artifact.mime_type,
            size: record.size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(session: VizierSession, dream_sources: Vec<VizierSession>) -> ToolContext {
        ToolContext {
            session,
            pending_attachments: Default::default(),
            hooks: None,
            background_depth: 0,
            loaded_images: Default::default(),
            recalled: Default::default(),
            deadline: None,
            client: None,
            secret_redactor: Default::default(),
            dream_sources,
        }
    }

    fn web(topic: &str) -> VizierSession {
        VizierSession(
            "a".into(),
            VizierChannelId::HTTP("ana".into(), "vizier-webui".into()),
            Some(topic.into()),
        )
    }

    #[test]
    fn an_ordinary_turn_reads_only_its_own_session() {
        let current = web("t");
        let c = ctx(current.clone(), vec![]);
        assert_eq!(resolve_source_session(&c, None).unwrap(), current);
        assert_eq!(
            resolve_source_session(&c, Some(&current.to_slug())).unwrap(),
            current
        );
        assert!(resolve_source_session(&c, Some("other__x__y")).is_err());
    }

    #[test]
    fn a_dream_uses_only_its_source_allow_list() {
        let dream = VizierSession(
            "a".into(),
            VizierChannelId::Dream(Box::new(web("s")), crate::schema::DreamStage::Extraction),
            Some("cycle".into()),
        );
        let sources = vec![web("s1"), web("s2")];
        let c = ctx(dream, sources.clone());
        assert_eq!(
            resolve_source_session(&c, Some(&sources[0].to_slug())).unwrap(),
            sources[0]
        );
        assert!(resolve_source_session(&c, Some(&web("nope").to_slug())).is_err());
        // Several sources and no name: refuse rather than guess.
        assert!(resolve_source_session(&c, None).is_err());
    }

    #[test]
    fn a_dream_with_one_source_defaults_to_it() {
        let source = web("s");
        let dream = VizierSession(
            "a".into(),
            VizierChannelId::Dream(
                Box::new(source.clone()),
                crate::schema::DreamStage::Extraction,
            ),
            Some("cycle".into()),
        );
        let c = ctx(dream, vec![source.clone()]);
        assert_eq!(resolve_source_session(&c, None).unwrap(), source);
    }
}
