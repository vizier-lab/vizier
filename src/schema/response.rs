use anyhow::Result;
use base64::Engine;
use chrono::{DateTime, Utc};
use rig::{
    OneOrMany,
    message::{ImageMediaType, MimeType, ToolResultContent, UserContent},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

use crate::{
    error::VizierError,
    schema::{VizierAttachment, VizierAttachmentContent},
    utils::get_mime_type,
};

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
pub struct VizierResponseStats {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub total_cached_input_tokens: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    pub duration: tokio::time::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
pub struct VizierResponse {
    pub timestamp: DateTime<Utc>,
    pub content: VizierResponseContent,
    pub attachments: Vec<VizierAttachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VizierResponseContent {
    ThinkingStart,
    Thinking(String),
    ToolChoice {
        name: String,
        args: serde_json::Value,
    },
    ToolResponse {
        response: serde_json::Value,
    },
    Message {
        content: String,
        stats: Option<VizierResponseStats>,
    },
    Empty,
    Abort,
}

impl VizierAttachment {
    pub fn to_tool_result_content(&self) -> Result<ToolResultContent> {
        let attachment = self.clone();
        let mime_type = get_mime_type(&attachment.filename);
        let content = if mime_type.starts_with("image/") {
            let media_type = ImageMediaType::from_mime_type(&mime_type).ok_or_else(|| {
                VizierError(format!("Unsupported image MIME type: {}", mime_type))
            })?;
            match &attachment.content {
                VizierAttachmentContent::Bytes(bytes) => {
                    let base64 = base64::engine::general_purpose::STANDARD.encode(bytes);

                    ToolResultContent::image_base64(base64, Some(media_type), None)
                }
                VizierAttachmentContent::Url(url) => {
                    ToolResultContent::image_url(url, Some(media_type), None)
                }
                VizierAttachmentContent::Base64(base64) => {
                    ToolResultContent::image_base64(base64, Some(media_type), None)
                }
            }
        } else {
            match &attachment.content {
                _ => unimplemented!(),
            }
        };

        Ok(content)
    }
}

impl VizierResponse {
    pub fn to_tool_response_content(
        &self,
        id: String,
        call_id: Option<String>,
    ) -> Result<UserContent> {
        let mut tool_contents = match &self.content {
            VizierResponseContent::Message { content, stats: _ } => {
                vec![ToolResultContent::text(content)]
            }

            VizierResponseContent::ToolResponse { response } => {
                vec![ToolResultContent::text(serde_json::to_string(&response)?)]
            }

            _ => return Err(VizierError("unimplemented".into()).into()),
        };

        // handle attachment
        for attachment in &self.attachments {
            tool_contents.push(attachment.to_tool_result_content()?);
        }

        let res = if let Some(call_id) = call_id {
            UserContent::tool_result_with_call_id(id, call_id, OneOrMany::many(tool_contents)?)
        } else {
            UserContent::tool_result(id, OneOrMany::many(tool_contents)?)
        };

        Ok(res)
    }
}
