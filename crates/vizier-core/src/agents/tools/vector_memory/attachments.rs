//! Memory attachment tools (`specs/035-add-artifacts` contracts/agent-tools-and-memory.md §2).
//!
//! These are the only way a new attachment gets onto a memory. `memory_write` is content-only
//! and preserves whatever is already attached.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    schema::RevisionOrigin,
    storage::{
        VizierStorage, artifact::ArtifactStorage, memory::MemoryStorage,
        session_file::SessionFileStorage,
    },
};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AttachmentList {
    /// Display names of the memory's attachments after the operation.
    pub attachments: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AttachArtifactArgs {
    #[schemars(description = "bundle name; omit for your default bundle")]
    #[serde(default)]
    pub bundle: Option<String>,
    #[schemars(description = "the memory's path within its bundle, without .md")]
    pub path: String,
    #[schemars(description = "name of one of this agent's artifacts")]
    pub artifact: String,
}

pub struct AttachArtifactToMemory {
    pub agent_id: String,
    pub storage: Arc<VizierStorage>,
}

#[async_trait::async_trait]
impl VizierTool for AttachArtifactToMemory {
    type Input = AttachArtifactArgs;
    type Output = AttachmentList;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Memory)
    }

    fn name() -> String {
        "attach_artifact_to_memory".into()
    }

    fn description(&self) -> String {
        "Attach one permanent artifact to an existing memory. Idempotent: attaching the same \
         artifact again changes nothing. Only artifact names work — session files must be \
         promoted with `promote_artifact` first. Remove one with \
         `detach_attachment_from_memory`."
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let artifact = self
            .storage
            .get_artifact_by_name(&self.agent_id, &args.artifact)
            .await
            .map_err(|e| VizierError(e.to_string()))?;
        let Some(artifact) = artifact else {
            if self
                .storage
                .get_session_file(&ctx.session, &args.artifact)
                .await
                .ok()
                .flatten()
                .is_some()
            {
                return Err(VizierError(format!(
                    "'{}' is a session file, not an artifact. Promote it with `promote_artifact` first.",
                    args.artifact
                )));
            }
            return Err(VizierError(format!(
                "no artifact named '{}' on this agent",
                args.artifact
            )));
        };
        let memory = self
            .storage
            .attach_memory_attachment(
                self.agent_id.clone(),
                args.bundle,
                args.path,
                artifact.id,
                artifact.name,
                &RevisionOrigin::from_session(&ctx.session),
            )
            .await
            .map_err(|e| VizierError(e.to_string()))?;
        Ok(AttachmentList {
            attachments: memory
                .attachments
                .iter()
                .map(|a| a.display_name().to_string())
                .collect(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DetachAttachmentArgs {
    #[schemars(description = "bundle name; omit for your default bundle")]
    #[serde(default)]
    pub bundle: Option<String>,
    #[schemars(description = "the memory's path within its bundle, without .md")]
    pub path: String,
    #[schemars(description = "display name of the attachment to remove")]
    pub name: String,
}

pub struct DetachAttachmentFromMemory {
    pub agent_id: String,
    pub storage: Arc<VizierStorage>,
}

#[async_trait::async_trait]
impl VizierTool for DetachAttachmentFromMemory {
    type Input = DetachAttachmentArgs;
    type Output = AttachmentList;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Memory)
    }

    fn name() -> String {
        "detach_attachment_from_memory".into()
    }

    fn description(&self) -> String {
        "Remove one attachment from a memory by display name — an artifact or an older \
         session-file attachment. A name that is not attached changes nothing and returns the \
         current list. The memory body is never touched."
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let memory = self
            .storage
            .detach_memory_attachment(
                self.agent_id.clone(),
                args.bundle,
                args.path,
                args.name,
                &RevisionOrigin::from_session(&ctx.session),
            )
            .await
            .map_err(|e| VizierError(e.to_string()))?;
        Ok(AttachmentList {
            attachments: memory
                .attachments
                .iter()
                .map(|a| a.display_name().to_string())
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_names_and_categories_are_stable() {
        assert_eq!(AttachArtifactToMemory::name(), "attach_artifact_to_memory");
        assert_eq!(
            AttachArtifactToMemory::category(),
            Some(ToolCategory::Memory)
        );
        assert_eq!(
            DetachAttachmentFromMemory::name(),
            "detach_attachment_from_memory"
        );
        assert_eq!(
            DetachAttachmentFromMemory::category(),
            Some(ToolCategory::Memory)
        );
    }
}
