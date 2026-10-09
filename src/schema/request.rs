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


use crate::{error::VizierError, schema::BackgroundReport, utils::get_mime_type};

#[derive(
    Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema, PartialEq,
)]
#[serde(rename_all = "snake_case")]
pub enum PlatformMessageId {
    Discord(u64),
    Telegram(i64),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReactionAction {
    Added,
    Removed,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema, PartialEq,
)]
pub struct ReactionEntry {
    pub user_id: String,
    pub emoji: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VizierRequestContent {
    Chat(String),
    Prompt(String),
    SilentRead(String),
    /// A machine wrote this prompt and nobody is waiting on the answer — a
    /// scheduled task run, or a dream cycle's own work. Deliberately not named
    /// `Task`: two of its three construction sites are the dream cycle, and the
    /// old name invited selecting scheduled-run behaviour on the content kind
    /// rather than on the session's channel.
    Unattended(String),
    Command(String),
    AudioChat(VizierAttachment, Option<String>),
    AudioPrompt(VizierAttachment, Option<String>),
    /// The outcome of background work this conversation started (`paralel_subtasks`,
    /// `delegate_agent`), delivered as a turn of its own. Not a message from a person.
    BackgroundReport(BackgroundReport),
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
            Self::Unattended(content) => write!(f, "{}", content),
            Self::Command(content) => write!(f, "{}", content),
            Self::AudioChat(att, transcription) => match transcription {
                Some(text) => write!(f, "{}", text),
                None => write!(f, "Voice message ({})", att.filename),
            },
            Self::AudioPrompt(att, transcription) => match transcription {
                Some(text) => write!(f, "{}", text),
                None => write!(f, "Voice message ({})", att.filename),
            },
            Self::BackgroundReport(report) => write!(f, "{}", report),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VizierAttachmentContent {
    Bytes(Vec<u8>),
    Base64(String),
    Url(String),
    Local(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
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
    Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema, Default,
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
    #[serde(default)]
    pub expect_audio_reply: Option<bool>,
    /// The slug of the task this request is a scheduled run of.
    ///
    /// Set by the scheduler and by nothing else, which is what makes it safe where the
    /// request content kind is not: the dream cycle sends `Unattended` too and must not be
    /// attributed to the scheduler. `Default` leaves it `None` for every other construction
    /// site, so no interactive turn's frontmatter changes.
    #[serde(default)]
    pub scheduled_task: Option<String>,
    /// How many background hops led to this turn: 0 for anything a person or the scheduler
    /// started, `job.depth + 1` for a background piece or a background report. Carried on the
    /// request rather than the session, because a woken turn runs in the same session as the
    /// turn that launched the job and must still count as one level deeper.
    #[serde(default)]
    pub background_depth: u8,
}

impl VizierRequest {
    pub fn to_prompt(&self) -> anyhow::Result<String> {
        let mut prompt = format!(
            "---\n{}\n---\n\n{}",
            self.generate_frontmatter()?,
            self.content
        );

        let mut all_attachments_info = vec![];

        for a in &self.attachments {
            let mime = get_mime_type(&a.filename);
            all_attachments_info.push(format!("- {} ({})", a.filename, mime));
        }

        if !all_attachments_info.is_empty() {
            prompt = format!(
                "{}\n\n# Attached Files\n{}\nthe following files added to your session files.\nUse `read_document_file` for documents and `read_image_file` for images.",
                prompt,
                all_attachments_info.join("\n")
            );
        }

        Ok(prompt)
    }

    pub fn generate_frontmatter(&self) -> anyhow::Result<String> {
        // A scheduled run is attributed to the scheduler on behalf of the requester. It
        // used to emit the task's stored `user` as `sender`, so the run arrived looking
        // like a message a named person had just sent — and with BOOT.md directive 4
        // telling the agent to check the channel metadata to understand the interaction,
        // that name was the only social cue in the request. Models read it correctly and
        // answered conversationally.
        //
        // Who the work is *for* is kept: a report written for a specific person can be
        // written for them. What is dropped is the false claim that they sent it.
        if let Some(task) = &self.scheduled_task {
            return Ok(serde_yaml::to_string(&json!({
                "sender": "scheduler",
                "task": task,
                "requested_by": self.user,
                "metadata": self.metadata,
            }))?);
        }

        // A background report is machine-written too, and is attributed as such for the same
        // reason: its `user` is the agent's own id, and nobody sent it.
        if let VizierRequestContent::BackgroundReport(report) = &self.content {
            return Ok(serde_yaml::to_string(&json!({
                "sender": "background",
                "job": report.job_id,
                "job_kind": report.kind.as_str(),
                "metadata": self.metadata,
            }))?);
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn scheduled() -> VizierRequest {
        VizierRequest {
            timestamp: Utc::now(),
            user: "@dani (DiscordId: 182)".to_string(),
            content: VizierRequestContent::Unattended("Summarise merged PRs".to_string()),
            metadata: serde_json::json!({ "timestamp": "2026-10-04T09:00:00Z" }),
            scheduled_task: Some("daily-report".to_string()),
            ..Default::default()
        }
    }

    /// A scheduled run is attributed to the scheduler on behalf of the requester. Before
    /// this, the frontmatter emitted the task's stored `user` as `sender`, so the run
    /// arrived looking like a message a named person had just sent — the only social cue in
    /// a request that otherwise offers nothing to check.
    #[test]
    fn a_scheduled_run_is_sent_by_the_scheduler_on_someones_behalf() {
        let frontmatter = scheduled().generate_frontmatter().unwrap();

        assert!(frontmatter.contains("sender: scheduler"), "{frontmatter}");
        assert!(frontmatter.contains("task: daily-report"), "{frontmatter}");
        // Who the work is *for* is kept: a report written for a specific person can be
        // written for them. What is dropped is the claim that they sent it.
        assert!(
            frontmatter.contains("requested_by: '@dani (DiscordId: 182)'"),
            "{frontmatter}"
        );
        assert!(
            !frontmatter.contains("sender: '@dani"),
            "the requester must not appear as the sender: {frontmatter}"
        );
    }

    /// An agent's own initiative renders as `self`, which is what `Requester::Agent` maps to.
    #[test]
    fn an_agents_own_initiative_is_requested_by_self() {
        let mut req = scheduled();
        req.user = "self".to_string();

        let frontmatter = req.generate_frontmatter().unwrap();
        assert!(frontmatter.contains("requested_by: self"), "{frontmatter}");
    }

    /// Every other turn's frontmatter is untouched. `scheduled_task` is `None` by `Default`
    /// and set only by the scheduler, so no interactive turn and no dream request can reach
    /// the scheduled branch — which is the same reason the framing is selected on the
    /// session's channel rather than on this request's content kind.
    #[test]
    fn an_interactive_turn_still_names_its_sender() {
        let req = VizierRequest {
            timestamp: Utc::now(),
            user: "someone".to_string(),
            content: VizierRequestContent::Chat("hey".to_string()),
            ..Default::default()
        };

        let frontmatter = req.generate_frontmatter().unwrap();
        assert!(frontmatter.contains("sender: someone"), "{frontmatter}");
        assert!(!frontmatter.contains("scheduler"), "{frontmatter}");
        assert!(!frontmatter.contains("requested_by"), "{frontmatter}");
    }

    /// A background report is sent by the background machinery, never by a person — its
    /// `user` is the agent's own id and must not appear as the sender.
    #[test]
    fn a_background_report_is_sent_by_the_background() {
        let req = VizierRequest {
            timestamp: Utc::now(),
            user: "agent-1".to_string(),
            content: VizierRequestContent::BackgroundReport(BackgroundReport {
                job_id: "b-7f3a9c".to_string(),
                kind: crate::schema::JobKind::Batch,
                delegated_to: None,
                entries: vec![],
            }),
            metadata: serde_json::json!({}),
            background_depth: 1,
            ..Default::default()
        };

        let frontmatter = req.generate_frontmatter().unwrap();
        assert!(frontmatter.contains("sender: background"), "{frontmatter}");
        assert!(frontmatter.contains("job: b-7f3a9c"), "{frontmatter}");
        assert!(frontmatter.contains("job_kind: batch"), "{frontmatter}");
        assert!(!frontmatter.contains("agent-1"), "{frontmatter}");
    }

    /// A dream request carries `Unattended` too, and must not be attributed to the
    /// scheduler — the content kind is not the discriminator anywhere in this feature.
    #[test]
    fn a_dream_request_is_not_attributed_to_the_scheduler() {
        let req = VizierRequest {
            timestamp: Utc::now(),
            user: "agent-1".to_string(),
            content: VizierRequestContent::Unattended("extract insights".to_string()),
            ..Default::default()
        };

        let frontmatter = req.generate_frontmatter().unwrap();
        assert!(frontmatter.contains("sender: agent-1"), "{frontmatter}");
        assert!(!frontmatter.contains("scheduler"), "{frontmatter}");
    }
}
