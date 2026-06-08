use std::fmt::Display;
use std::path::PathBuf;

use anyhow::Result;
use base64::Engine;
use chrono::{DateTime, Utc};
use rig_core::{
    OneOrMany,
    message::{DocumentMediaType, ImageMediaType, Message, MimeType, UserContent},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use surrealdb_types::SurrealValue;

use crate::{error::VizierError, utils::get_mime_type};

#[derive(
    Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema, PartialEq,
)]
#[serde(rename_all = "snake_case")]
pub enum PlatformMessageId {
    Discord(u64),
    Telegram(i64),
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReactionAction {
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
pub struct ReactionEvent {
    #[serde(default)]
    pub platform_message_id: Option<PlatformMessageId>,
    pub user_id: String,
    pub emoji: String,
    pub action: ReactionAction,
}

impl ReactionEvent {
    pub fn action_str(&self) -> &str {
        match self.action {
            ReactionAction::Added => "added",
            ReactionAction::Removed => "removed",
        }
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema, PartialEq,
)]
pub struct ReactionEntry {
    pub user_id: String,
    pub emoji: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VizierRequestContent {
    Chat(String),
    Prompt(String),
    SilentRead(String),
    Task(String),
    Command(String),
    Reaction(ReactionEvent),
    AudioChat(VizierAttachment, Option<String>),
    AudioPrompt(VizierAttachment, Option<String>),
}

impl Default for VizierRequestContent {
    fn default() -> Self {
        Self::Prompt("".to_string())
    }
}

impl Display for VizierRequestContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Chat(content) => write!(f, "{}", content),
            Self::Prompt(content) => write!(f, "{}", content),
            Self::SilentRead(content) => write!(f, "{}", content),
            Self::Task(content) => write!(f, "{}", content),
            Self::Command(content) => write!(f, "{}", content),
            Self::Reaction(event) => {
                write!(
                    f,
                    "Reaction: {} {} by {}",
                    event.action_str(),
                    event.emoji,
                    event.user_id
                )
            }
            Self::AudioChat(att, transcription) => {
                match transcription {
                    Some(text) => write!(f, "{}", text),
                    None => write!(f, "Voice message ({})", att.filename),
                }
            }
            Self::AudioPrompt(att, transcription) => {
                match transcription {
                    Some(text) => write!(f, "{}", text),
                    None => write!(f, "Voice message ({})", att.filename),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VizierAttachmentContent {
    Bytes(Vec<u8>),
    Base64(String),
    Url(String),
    Local(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
pub struct VizierAttachment {
    pub filename: String,
    pub content: VizierAttachmentContent,
}

impl VizierAttachment {
    pub fn to_user_content(&self, workspace: &str) -> Result<UserContent> {
        let attachment = self.clone();
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
                VizierAttachmentContent::Local(path) => {
                    let bytes = Self::resolve_local_bytes(workspace, path)?;
                    let base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    UserContent::image_base64(base64, Some(media_type), None)
                }
            }
        } else {
            let media_type = DocumentMediaType::from_mime_type(&mime_type).ok_or_else(|| {
                VizierError(format!("Unsupported image MIME type: {}", mime_type))
            })?;

            match &attachment.content {
                VizierAttachmentContent::Bytes(bytes) => {
                    UserContent::document_raw(bytes.clone(), Some(media_type))
                }
                VizierAttachmentContent::Url(url) => {
                    UserContent::document_url(url.clone(), Some(media_type))
                }
                VizierAttachmentContent::Local(path) => {
                    let bytes = Self::resolve_local_bytes(workspace, path)?;
                    UserContent::document_raw(bytes, Some(media_type))
                }
                _ => unimplemented!(),
            }
        };

        Ok(content)
    }

    pub fn resolve_local_bytes(workspace: &str, path: &str) -> Result<Vec<u8>> {
        let file_id = path.trim_start_matches("/api/v1/files/");
        let uploads_dir = PathBuf::from(workspace).join("uploads").join(file_id);
        let mut entries = std::fs::read_dir(&uploads_dir).map_err(|e| {
            VizierError(format!(
                "Failed to read uploads dir {}: {}",
                uploads_dir.display(),
                e
            ))
        })?;
        let file_path = entries
            .next()
            .and_then(|r| r.ok())
            .ok_or_else(|| VizierError(format!("No file found in {}", uploads_dir.display())))?
            .path();
        Ok(std::fs::read(&file_path).map_err(|e| {
            VizierError(format!(
                "Failed to read local file {}: {}",
                file_path.display(),
                e
            ))
        })?)
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema, Default,
)]
pub struct VizierRequest {
    pub timestamp: DateTime<Utc>,
    pub user: String,
    pub content: VizierRequestContent,
    #[serde(default)]
    pub platform_message_id: Option<PlatformMessageId>,
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub attachments: Vec<VizierAttachment>,
}

impl VizierRequest {
    pub fn to_prompt(&self) -> anyhow::Result<String> {
        let mut prompt = format!(
            "---\n{}\n---\n\n{}",
            self.generate_frontmatter()?,
            self.content
        );

        let mut all_attachments_info = vec![];

        // Include audio attachment from AudioChat/AudioPrompt content
        if let VizierRequestContent::AudioChat(att, _)
        | VizierRequestContent::AudioPrompt(att, _) = &self.content
        {
            let mime = get_mime_type(&att.filename);
            all_attachments_info.push(format!("- {} ({})", att.filename, mime));
        }

        for a in &self.attachments {
            let mime = get_mime_type(&a.filename);
            all_attachments_info.push(format!("- {} ({})", a.filename, mime));
        }

        if !all_attachments_info.is_empty() {
            prompt = format!(
                "{}\n\n# Attached Files\n{}\nthe following files added to your session files.\nUse read_session_file to access these files.",
                prompt,
                all_attachments_info.join("\n")
            );
        }

        Ok(prompt)
    }

    pub fn generate_frontmatter(&self) -> anyhow::Result<String> {
        Ok(serde_yaml::to_string(&json!({
            "sender": self.user,
            "metadata": self.metadata,
        }))?)
    }

    pub fn to_message(&self, workspace: &str) -> Result<Message> {
        let mut contents = vec![UserContent::Text(
            self.to_prompt()
                .map_err(|err| VizierError(err.to_string()))?
                .into(),
        )];
        for attachment in self.attachments.iter() {
            contents.push(attachment.to_user_content(workspace)?);
        }

        let message = Message::User {
            content: OneOrMany::many(contents).unwrap(),
        };

        Ok(message)
    }
}
