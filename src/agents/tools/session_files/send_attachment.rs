use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    schema::{VizierAttachment, VizierAttachmentContent},
    storage::{VizierStorage, artifact::ArtifactStorage, session_file::SessionFileStorage},
};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SendAttachmentArgs {
    #[schemars(description = "name of the artifact to send as an attachment")]
    pub artifact: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SendAttachmentOutput {
    pub filename: String,
    pub status: String,
}

pub struct SendAttachment {
    pub storage: Arc<VizierStorage>,
}

#[async_trait::async_trait]
impl VizierTool for SendAttachment {
    type Input = SendAttachmentArgs;
    type Output = SendAttachmentOutput;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Files)
    }

    fn name() -> String {
        "send_attachment".to_string()
    }

    fn description(&self) -> String {
        "Queue one of this agent's permanent artifacts to be sent with your response. Session files are transient: promote one with `promote_artifact` first. The current artifact content is served at send time, so a later update or deletion is reflected."
            .to_string()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let artifact = self
            .storage
            .get_artifact_by_name(&ctx.session.0, &args.artifact)
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

        ctx.pending_attachments.lock().await.push(VizierAttachment {
            filename: artifact.name.clone(),
            content: VizierAttachmentContent::Artifact {
                agent_id: artifact.agent_id.clone(),
                artifact_id: artifact.id.clone(),
                available: None,
            },
        });

        Ok(SendAttachmentOutput {
            filename: artifact.name,
            status: "sent".into(),
        })
    }
}
