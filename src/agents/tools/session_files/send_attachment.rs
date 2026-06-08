use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolContext, VizierTool},
    error::VizierError,
    schema::{VizierAttachment, VizierAttachmentContent},
    storage::{VizierStorage, session_file::SessionFileStorage},
};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SendAttachmentArgs {
    #[schemars(description = "filename of the session file to send as attachment")]
    pub filename: String,
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

    fn name() -> String {
        "send_attachment".to_string()
    }

    fn description(&self) -> String {
        "Queue a session file to be sent back to the user as an attachment with your response."
            .to_string()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let file = self
            .storage
            .get_session_file(&ctx.session, &args.filename)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .ok_or_else(|| VizierError(format!("File not found: {}", args.filename)))?;

        let url = format!("/api/v1/files/{}", file.file_id);

        ctx.pending_attachments.lock().await.push(VizierAttachment {
            filename: file.filename.clone(),
            content: VizierAttachmentContent::Local(url),
        });

        Ok(SendAttachmentOutput {
            filename: file.filename,
            status: "sent".into(),
        })
    }
}
