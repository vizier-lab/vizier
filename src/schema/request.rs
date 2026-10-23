use std::fmt::Display;
use std::path::PathBuf;

use anyhow::Result;
use base64::Engine;
use chrono::{DateTime, Utc};
use crate::llm::types::{Message, Part};
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
    /// A message a client sent on the person's behalf, such as a click inside a page the
    /// agent drew (`specs/019-client-tools`). It starts a turn like `Chat`.
    ClientEvent(ClientEvent),
}

/// What a client event carries: the text shown as the person's message, data only the
/// agent sees, and the client tool call it came from.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema, PartialEq)]
pub struct ClientEvent {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_call_id: Option<String>,
}

impl Display for ClientEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)?;
        if let Some(data) = &self.data {
            let data = serde_json::to_string(data).unwrap_or_default();
            write!(f, "\n\n```json\n{data}\n```")?;
        }
        Ok(())
    }
}

impl VizierRequestContent {
    /// The variant's name, as it serialises.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Chat(_) => "chat",
            Self::Prompt(_) => "prompt",
            Self::SilentRead(_) => "silent_read",
            Self::Unattended(_) => "unattended",
            Self::Command(_) => "command",
            Self::AudioChat(..) => "audio_chat",
            Self::AudioPrompt(..) => "audio_prompt",
            Self::BackgroundReport(_) => "background_report",
            Self::ClientEvent(_) => "client_event",
        }
    }
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
            Self::ClientEvent(event) => write!(f, "{}", event),
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
    /// The attachment as a message part. Whether the model gets it, or a note saying it was
    /// attached, is decided by capability in `llm::prepare` (FR-013).
    pub fn to_part(&self, workspace: &str) -> Result<Part> {
        let mime = get_mime_type(&self.filename);
        let data = match &self.content {
            VizierAttachmentContent::Bytes(bytes) => bytes.clone(),
            VizierAttachmentContent::Base64(b64) => base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| VizierError(format!("attachment {}: {e}", self.filename)))?,
            VizierAttachmentContent::Local(path) => Self::resolve_local_bytes(workspace, path)?,
            VizierAttachmentContent::Url(url) => {
                return Ok(Part::Text(format!("[attachment {}: {url}]", self.filename)));
            }
        };
        Ok(Part::Attachment {
            mime,
            data,
            name: Some(self.filename.clone()),
        })
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
    /// The trace this request continues (`crate::telemetry`); in-process only.
    #[serde(skip)]
    #[schemars(skip)]
    #[schema(ignore)]
    pub trace: crate::telemetry::RequestTrace,
    /// Tools the sending client offers for this one turn (`specs/019-client-tools`). Read
    /// off the WebSocket frame and moved into `client` once checked, so they never reach a
    /// stored request.
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    #[schema(ignore)]
    pub client_tools: Vec<crate::agents::client::ClientToolDeclaration>,
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    #[schema(ignore)]
    pub client_categories: Vec<crate::agents::client::ClientCategoryDeclaration>,
    /// The checked declarations and the connection that answers them; in-process only.
    #[serde(skip)]
    #[schemars(skip)]
    #[schema(ignore)]
    pub client: Option<crate::agents::client::ClientBinding>,
}

impl VizierRequest {
    /// Whether the request is journaled so a restart can pick it up again. An abort is an
    /// instruction about the present, and a dream cycle is rescheduled on its own.
    pub fn is_durable(&self, session: &crate::schema::VizierSession) -> bool {
        if matches!(&self.content, VizierRequestContent::Command(cmd) if cmd == "abort" || cmd == "dream") {
            return false;
        }
        !matches!(session.1, crate::schema::VizierChannelId::Dream(..))
    }

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

        // A client event is still the person's message (they clicked it), so `sender` stays
        // theirs; `via` says it did not come from typing.
        if let VizierRequestContent::ClientEvent(event) = &self.content {
            let mut frontmatter = serde_json::Map::new();
            frontmatter.insert("sender".into(), json!(self.user));
            frontmatter.insert("via".into(), json!("client_event"));
            if let Some(source) = &event.source_call_id {
                frontmatter.insert("source_call".into(), json!(source));
            }
            frontmatter.insert("metadata".into(), self.metadata.clone());
            return Ok(serde_yaml::to_string(&frontmatter)?);
        }

        Ok(serde_yaml::to_string(&json!({
            "sender": self.user,
            "metadata": self.metadata,
        }))?)
    }

    pub fn to_message(&self, workspace: &str) -> Result<Message> {
        let mut contents = vec![Part::Text(
            self.to_prompt()
                .map_err(|err| VizierError(err.to_string()))?,
        )];
        for attachment in self.attachments.iter() {
            contents.push(attachment.to_part(workspace)?);
        }
        Ok(Message::User(contents))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn abort_dream_and_dream_sessions_are_not_durable() {
        use crate::schema::{DreamStage, VizierChannelId, VizierSession};
        let session = VizierSession("a".into(), VizierChannelId::System, None);
        let req = |content| VizierRequest { content, ..Default::default() };
        assert!(req(VizierRequestContent::Chat("hi".into())).is_durable(&session));
        assert!(req(VizierRequestContent::Command("checkpoint".into())).is_durable(&session));
        assert!(!req(VizierRequestContent::Command("abort".into())).is_durable(&session));
        assert!(!req(VizierRequestContent::Command("dream".into())).is_durable(&session));
        let dream = VizierSession(
            "a".into(),
            VizierChannelId::Dream(Box::new(session.clone()), DreamStage::Extraction),
            None,
        );
        assert!(!req(VizierRequestContent::Chat("x".into())).is_durable(&dream));
    }

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

    fn client_event(source: Option<&str>, data: Option<serde_json::Value>) -> VizierRequest {
        VizierRequest {
            timestamp: Utc::now(),
            user: "alice".to_string(),
            content: VizierRequestContent::ClientEvent(ClientEvent {
                text: "Submitted filters".into(),
                data,
                source_call_id: source.map(str::to_string),
            }),
            metadata: serde_json::json!({}),
            ..Default::default()
        }
    }

    /// The person stays the sender of a client event; `via` and `source_call` say where it
    /// came from (specs/019-client-tools R10).
    #[test]
    fn a_client_event_is_the_persons_message_via_the_client() {
        let frontmatter = client_event(Some("call-1"), None).generate_frontmatter().unwrap();
        assert!(frontmatter.contains("sender: alice"), "{frontmatter}");
        assert!(frontmatter.contains("via: client_event"), "{frontmatter}");
        assert!(frontmatter.contains("source_call: call-1"), "{frontmatter}");

        let frontmatter = client_event(None, None).generate_frontmatter().unwrap();
        assert!(!frontmatter.contains("source_call"), "{frontmatter}");
    }

    #[test]
    fn a_client_event_reads_as_its_text_then_its_data() {
        let req = client_event(None, Some(serde_json::json!({ "quarter": 3 })));
        assert_eq!(req.content.to_string(), "Submitted filters\n\n```json\n{\"quarter\":3}\n```");
        assert_eq!(req.content.kind(), "client_event");
        assert_eq!(client_event(None, None).content.to_string(), "Submitted filters");
    }

    /// Declarations and the binding are never written out, so no stored request holds them.
    #[test]
    fn a_serialised_request_omits_the_client_fields() {
        let req: VizierRequest = serde_json::from_value(serde_json::json!({
            "timestamp": "2026-10-17T12:00:00Z",
            "user": "alice",
            "content": { "chat": "hi" },
            "metadata": {},
            "client_tools": [{ "name": "pick", "description": "d", "input_schema": { "type": "object" } }],
            "client_categories": [{ "name": "client" }],
        }))
        .unwrap();
        assert_eq!(req.client_tools.len(), 1);
        assert_eq!(req.client_categories.len(), 1);
        let out = serde_json::to_value(&req).unwrap();
        for key in ["client_tools", "client_categories", "client"] {
            assert!(out.get(key).is_none(), "{key} was serialised: {out}");
        }
    }
}
