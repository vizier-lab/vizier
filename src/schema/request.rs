use std::fmt::Display;

use anyhow::Result;
use base64::Engine;
use chrono::{DateTime, Utc};
use rig::{
    OneOrMany,
    message::{DocumentMediaType, ImageMediaType, Message, MimeType, UserContent},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use surrealdb_types::SurrealValue;

use crate::{error::VizierError, utils::get_mime_type};

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VizierRequestContent {
    Chat(String),
    Prompt(String),
    SilentRead(String),
    Task(String),
    Command(String),
}

impl Default for VizierRequestContent {
    fn default() -> Self {
        Self::Prompt("".to_string())
    }
}

impl Display for VizierRequestContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Chat(content) => content,
                Self::Prompt(content) => content,
                Self::SilentRead(content) => content,
                Self::Task(content) => content,
                Self::Command(content) => content,
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
pub enum VizierAttachmentContent {
    Bytes(Vec<u8>),
    Base64(String),
    Url(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
pub struct VizierAttachment {
    pub filename: String,
    pub content: VizierAttachmentContent,
}

impl VizierRequest {
    pub fn to_message(&self) -> Result<Message> {
        let mut contents = vec![UserContent::Text(
            self.to_prompt()
                .map_err(|err| VizierError(err.to_string()))?
                .into(),
        )];
        for attachment in self.attachments.iter() {
            let mime_type = get_mime_type(&attachment.filename);
            let content = if mime_type.starts_with("image/") {
                let media_type = ImageMediaType::from_mime_type(&mime_type).ok_or_else(|| {
                    VizierError(format!("Unsupported image MIME type: {}", mime_type))
                })?;
                match &attachment.content {
                    VizierAttachmentContent::Bytes(bytes) => {
                        let base64 = base64::engine::general_purpose::STANDARD.encode(bytes);

                        UserContent::image_base64(base64, Some(media_type), None)
                    }
                    VizierAttachmentContent::Url(url) => {
                        UserContent::image_url(url, Some(media_type), None)
                    }
                    VizierAttachmentContent::Base64(base64) => {
                        UserContent::image_base64(base64, Some(media_type), None)
                    }
                }
            } else {
                let media_type =
                    DocumentMediaType::from_mime_type(&mime_type).ok_or_else(|| {
                        VizierError(format!("Unsupported image MIME type: {}", mime_type))
                    })?;

                match &attachment.content {
                    VizierAttachmentContent::Bytes(bytes) => {
                        UserContent::document_raw(bytes.clone(), Some(media_type))
                    }
                    _ => unimplemented!(),
                }
            };

            contents.push(content);
        }

        let message = Message::User {
            content: OneOrMany::many(contents).unwrap(),
        };

        Ok(message)
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema, Default,
)]
pub struct VizierRequest {
    pub timestamp: DateTime<Utc>,
    pub user: String,
    pub content: VizierRequestContent,
    pub metadata: serde_json::Value,
    pub attachments: Vec<VizierAttachment>,
}

impl VizierRequest {
    pub fn to_prompt(&self) -> anyhow::Result<String> {
        Ok(format!(
            "---\n{}\n---\n\n{}",
            self.generate_frontmatter()?,
            self.content
        ))
    }

    pub fn generate_frontmatter(&self) -> anyhow::Result<String> {
        Ok(serde_yaml::to_string(&json!({
            "sender": self.user,
            "metadata": self.metadata,
        }))?)
    }
}
