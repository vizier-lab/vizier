use std::fmt::Display;
use std::path::PathBuf;

use crate::llm::types::{Message, Part};
use anyhow::Result;
use base64::Engine;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{error::VizierError, schema::BackgroundReport, utils::get_mime_type};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum PlatformMessageId {
    Discord(u64),
    Telegram(i64),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ReactionAction {
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ReactionEntry {
    pub user_id: String,
    pub emoji: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
    /// The outcome of background work this conversation started (`background_subagents`,
    /// `delegate_agent`), delivered as a turn of its own. Not a message from a person.
    BackgroundReport(BackgroundReport),
    /// A message a client sent on the person's behalf, such as a click inside a page the
    /// agent drew (`specs/019-client-tools`). It starts a turn like `Chat`.
    ClientEvent(ClientEvent),
    /// An off-the-record `/btw` question (`specs/033-btw-side-chat`): the agent answers it
    /// from the conversation's context, but nothing above, about or around it is saved.
    /// Dispatched beside `checkpoint`/`lobotomy`, never through `handle_request`.
    Btw(SideChat),
}

/// One `/btw` side chat carried in each request: the question being asked now and the
/// aside's earlier turns, held between asks only by whoever shows the side chat (the WebUI
/// panel, or a Discord/Telegram reader). Never stored.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SideChat {
    pub question: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub turns: Vec<SideChatTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SideChatTurn {
    pub question: String,
    pub answer: String,
}

/// What a client event carries: the text shown as the person's message, data only the
/// agent sees, and the client tool call it came from.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
            Self::Btw(_) => "btw",
        }
    }

    /// The request is an off-the-record side chat. The one place this is decided, so the
    /// agent loop's two trace gates and the docs all test or read it from here.
    pub fn is_side_chat(&self) -> bool {
        matches!(self, Self::Btw(_))
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
            Self::Btw(side_chat) => write!(f, "{}", side_chat.question),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum VizierAttachmentContent {
    Bytes(Vec<u8>),
    Base64(String),
    Url(String),
    Local(String),
    /// An artifact by identity (`specs/035-add-artifacts`). The bytes are resolved through an
    /// [`AttachmentResolver`]; `available` is read-time only and never stored.
    Artifact {
        agent_id: String,
        artifact_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        available: Option<bool>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct VizierAttachment {
    pub filename: String,
    pub content: VizierAttachmentContent,
}

/// Turns an artifact identity into its current content path (`specs/035-add-artifacts` R3).
/// Synchronous because model-part conversion is; the implementation is a single indexed SQLite
/// read plus one content-file existence check.
pub trait AttachmentResolver: Send + Sync {
    fn artifact_path(&self, agent_id: &str, artifact_id: &str) -> Result<Option<PathBuf>>;
}

/// A resolver for tests and the offline provider: every artifact is unavailable.
pub struct NoopAttachmentResolver;

impl AttachmentResolver for NoopAttachmentResolver {
    fn artifact_path(&self, _agent_id: &str, _artifact_id: &str) -> Result<Option<PathBuf>> {
        Ok(None)
    }
}

impl VizierAttachment {
    /// The attachment as a message part. Whether the model gets it, or a note saying it was
    /// attached, is decided by capability in `llm::prepare` (FR-013).
    pub fn to_part(&self, workspace: &str, resolver: &dyn AttachmentResolver) -> Result<Part> {
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
            VizierAttachmentContent::Artifact {
                agent_id,
                artifact_id,
                ..
            } => match resolver.artifact_path(agent_id, artifact_id) {
                Ok(Some(path)) => std::fs::read(&path).map_err(|e| {
                    VizierError(format!(
                        "failed to read artifact {} at {}: {e}",
                        self.filename,
                        path.display()
                    ))
                })?,
                // A deleted artifact is a readable note, never a failed turn.
                _ => {
                    return Ok(Part::Text(format!(
                        "[attachment {}: artifact no longer available]",
                        self.filename
                    )));
                }
            },
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
    /// The project a subagent piece works in (`specs/041-project-coding-workspace` R11),
    /// copied from the launching turn by `BackgroundJobs::dispatch_piece`. It is serialised
    /// with the durable turn row, so a resumed piece keeps the folder and mode it started
    /// with; clients cannot send it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_scope: Option<crate::schema::ProjectScope>,
    /// Files and folders a project chat's message mentions (FR-018). Their contents are
    /// resolved when the turn starts and never stored.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<String>,
    /// A request that belongs to a blocking piece and must not survive a restart.
    /// Blocking pieces are never journaled or resumed; only their conversation history
    /// remains for inspection.
    #[serde(skip)]
    #[schemars(skip)]
    #[cfg_attr(feature = "utoipa", schema(ignore))]
    pub transient: bool,
    /// The trace this request continues (`crate::telemetry`); in-process only.
    #[serde(skip)]
    #[schemars(skip)]
    #[cfg_attr(feature = "utoipa", schema(ignore))]
    pub trace: crate::telemetry::RequestTrace,
    /// Tools the sending client offers for this one turn (`specs/019-client-tools`). Read
    /// off the WebSocket frame and moved into `client` once checked, so they never reach a
    /// stored request.
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    #[cfg_attr(feature = "utoipa", schema(ignore))]
    pub client_tools: Vec<crate::agents::client::ClientToolDeclaration>,
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    #[cfg_attr(feature = "utoipa", schema(ignore))]
    pub client_categories: Vec<crate::agents::client::ClientCategoryDeclaration>,
    /// The checked declarations and the connection that answers them; in-process only.
    #[serde(skip)]
    #[schemars(skip)]
    #[cfg_attr(feature = "utoipa", schema(ignore))]
    pub client: Option<crate::agents::client::ClientBinding>,
}

impl VizierRequest {
    /// Whether the request is journaled so a restart can pick it up again. An abort is an
    /// instruction about the present, and a dream cycle is rescheduled on its own.
    pub fn is_durable(&self, session: &crate::schema::VizierSession) -> bool {
        if self.transient {
            return false;
        }
        if matches!(&self.content, VizierRequestContent::Command(cmd) if cmd == "abort" || cmd == "dream")
        {
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
                "{}\n\n# Attached Files\n{}\nthe following files added to your session files.\nUse `read_document_file` for text documents and PDFs, `read_image_file` for images, and the `office_*` tools for Word and Excel files.",
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

    pub fn to_message(
        &self,
        workspace: &str,
        resolver: &dyn AttachmentResolver,
    ) -> Result<Message> {
        let mut contents = vec![Part::Text(
            self.to_prompt()
                .map_err(|err| VizierError(err.to_string()))?,
        )];
        for attachment in self.attachments.iter() {
            contents.push(attachment.to_part(workspace, resolver)?);
        }
        Ok(Message::User(contents))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn btw_serialises_with_its_question_and_earlier_turns() {
        let content = VizierRequestContent::Btw(SideChat {
            question: "what did I just say?".into(),
            turns: vec![SideChatTurn {
                question: "first".into(),
                answer: "first answer".into(),
            }],
        });
        let out = serde_json::to_value(&content).unwrap();
        assert_eq!(
            out,
            serde_json::json!({
                "btw": {
                    "question": "what did I just say?",
                    "turns": [{ "question": "first", "answer": "first answer" }],
                }
            })
        );
        // An empty turns list is omitted.
        let bare = VizierRequestContent::Btw(SideChat {
            question: "q".into(),
            turns: vec![],
        });
        assert_eq!(
            serde_json::to_value(&bare).unwrap(),
            serde_json::json!({ "btw": { "question": "q" } })
        );
    }

    #[test]
    fn the_btw_variant_is_named_btw_and_reads_as_its_question() {
        let side_chat = VizierRequestContent::Btw(SideChat {
            question: "off the record?".into(),
            turns: vec![],
        });
        assert_eq!(side_chat.kind(), "btw");
        assert_eq!(side_chat.to_string(), "off the record?");
        assert!(side_chat.is_side_chat());
    }

    /// The one predicate: every other content kind is an ordinary, recorded kind of turn.
    #[test]
    fn is_side_chat_is_true_only_for_btw() {
        for other in [
            VizierRequestContent::Chat("hi".into()),
            VizierRequestContent::Prompt("p".into()),
            VizierRequestContent::SilentRead("s".into()),
            VizierRequestContent::Unattended("u".into()),
            VizierRequestContent::Command("checkpoint".into()),
            VizierRequestContent::BackgroundReport(BackgroundReport {
                job_id: "j".into(),
                kind: crate::schema::JobKind::Batch,
                delegated_to: None,
                entries: vec![],
            }),
            VizierRequestContent::ClientEvent(ClientEvent {
                text: "x".into(),
                data: None,
                source_call_id: None,
            }),
            VizierRequestContent::Btw(SideChat {
                question: "q".into(),
                turns: vec![],
            }),
        ] {
            assert_eq!(other.is_side_chat(), matches!(other, VizierRequestContent::Btw(_)));
        }
    }

    /// A side chat is always sent transient (the one thing `side_chat::ask` builds), so no
    /// `session_turn` row is journaled for it.
    #[test]
    fn a_btw_request_is_not_durable() {
        use crate::schema::{VizierChannelId, VizierSession};
        let session = VizierSession("a".into(), VizierChannelId::System, None);
        let req = VizierRequest {
            content: VizierRequestContent::Btw(SideChat {
                question: "q".into(),
                turns: vec![],
            }),
            transient: true,
            ..Default::default()
        };
        assert!(!req.is_durable(&session));
    }

    #[test]
    fn abort_dream_and_dream_sessions_are_not_durable() {
        use crate::schema::{DreamStage, VizierChannelId, VizierSession};
        let session = VizierSession("a".into(), VizierChannelId::System, None);
        let req = |content| VizierRequest {
            content,
            ..Default::default()
        };
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

    #[test]
    fn a_transient_request_is_not_durable() {
        use crate::schema::{VizierChannelId, VizierSession};
        let session = VizierSession("a".into(), VizierChannelId::Subagent, None);
        let mut req = VizierRequest {
            content: VizierRequestContent::Prompt("piece".into()),
            transient: true,
            ..Default::default()
        };
        assert!(!req.is_durable(&session));
        req.transient = false;
        assert!(req.is_durable(&session));
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
        let frontmatter = client_event(Some("call-1"), None)
            .generate_frontmatter()
            .unwrap();
        assert!(frontmatter.contains("sender: alice"), "{frontmatter}");
        assert!(frontmatter.contains("via: client_event"), "{frontmatter}");
        assert!(frontmatter.contains("source_call: call-1"), "{frontmatter}");

        let frontmatter = client_event(None, None).generate_frontmatter().unwrap();
        assert!(!frontmatter.contains("source_call"), "{frontmatter}");
    }

    #[test]
    fn a_client_event_reads_as_its_text_then_its_data() {
        let req = client_event(None, Some(serde_json::json!({ "quarter": 3 })));
        assert_eq!(
            req.content.to_string(),
            "Submitted filters\n\n```json\n{\"quarter\":3}\n```"
        );
        assert_eq!(req.content.kind(), "client_event");
        assert_eq!(
            client_event(None, None).content.to_string(),
            "Submitted filters"
        );
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
