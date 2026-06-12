use std::sync::Arc;

use base64::Engine;
use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolContext, VizierTool},
    error::VizierError,
    file_manager::FileManager,
    schema::{VizierResponse, VizierResponseContent},
    storage::{VizierStorage, session_file::SessionFileStorage},
};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadImageFileArgs {
    #[schemars(description = "filename of the session image file to read")]
    pub filename: String,
}

pub struct ReadImageFile {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

#[async_trait::async_trait]
impl VizierTool for ReadImageFile {
    type Input = ReadImageFileArgs;
    type Output = VizierResponse;

    fn name() -> String {
        "read_image_file".to_string()
    }

    fn description(&self) -> String {
        "Read an image file from the current session and inject it into the conversation context as a vision attachment. The file must have an image/* MIME type; use read_document_file for textual documents, PDFs, and spreadsheets.".to_string()
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

        if !file.mime_type.starts_with("image/") {
            return Err(VizierError(format!(
                "read_image_file requires an image/* MIME type, got '{}'. Use read_document_file for this file.",
                file.mime_type
            )));
        }

        let (_, content) = self
            .file_manager
            .get(&file.file_id)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        let b64 = base64::engine::general_purpose::STANDARD.encode(&content);

        Ok(VizierResponse {
            timestamp: Utc::now(),
            content: VizierResponseContent::ToolResponse {
                response: serde_json::Value::String(format!(
                    "Loaded {} into context.",
                    file.filename
                )),
            },
            attachments: vec![crate::schema::VizierAttachment {
                filename: file.filename,
                content: crate::schema::VizierAttachmentContent::Base64(b64),
            }],
        })
    }
}
