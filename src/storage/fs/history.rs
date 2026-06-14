use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::{
    schema::{
        AgentUsageStats, ChannelTypeUsage, ChannelTypeUsageDetail, ChannelUsage,
        DailyChannelTypeUsage, DailyUsage, ErrorKind, ReactionEntry, SessionHistory,
        SessionHistoryContent, UsageSummary, VizierAttachment, VizierAttachmentContent,
        VizierRequest, VizierRequestContent, VizierResponse, VizierResponseContent,
        VizierResponseStats, VizierSession,
    },
    storage::{
        fs::{FileSystemStorage, HISTORY_PATH},
        history::HistoryStorage,
    },
    utils,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(non_camel_case_types)]
enum ContentMetadata {
    request {
        user: String,
        is_silent_read: bool,
        is_task: bool,
        is_chat: bool,
        is_prompt: bool,
        is_command: bool,
        is_audio_chat: bool,
        #[serde(default)]
        audio_message: Option<VizierAttachment>,
        #[serde(default)]
        audio_transcription: Option<String>,
        metadata: serde_json::Value,
        attachments: Vec<VizierAttachment>,
    },
    response {
        stats: Option<VizierResponseStats>,
        attachments: Vec<VizierAttachment>,
        #[serde(default)]
        is_audio_reply: bool,
        #[serde(default)]
        audio_reply: Option<VizierAttachment>,
        #[serde(default)]
        audio_reply_text: Option<String>,
        #[serde(default)]
        is_error: bool,
        #[serde(default)]
        error_kind: Option<ErrorKind>,
        #[serde(default)]
        error_message: Option<String>,
    },
    assistant_message {},
    tool_call {
        call_id: String,
        name: String,
    },
    tool_result {
        call_id: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SessionHistoryFrontMatter {
    pub uid: String,
    pub session: VizierSession,
    pub content_metadata: ContentMetadata,
    pub timestamp: chrono::DateTime<Utc>,
    #[serde(default)]
    pub reactions: Option<Vec<ReactionEntry>>,
}

impl From<SessionHistory> for SessionHistoryFrontMatter {
    fn from(value: SessionHistory) -> Self {
        Self {
            uid: value.uid,
            timestamp: value.timestamp,
            session: value.vizier_session,
            reactions: if value.reactions.is_empty() {
                None
            } else {
                Some(value.reactions)
            },
            content_metadata: match value.content {
                SessionHistoryContent::Request(req) => ContentMetadata::request {
                    user: req.user,
                    is_silent_read: if let VizierRequestContent::SilentRead(_) = req.content {
                        true
                    } else {
                        false
                    },
                    is_task: if let VizierRequestContent::Task(_) = req.content {
                        true
                    } else {
                        false
                    },
                    is_chat: if let VizierRequestContent::Chat(_) = req.content {
                        true
                    } else {
                        false
                    },
                    is_prompt: if let VizierRequestContent::Prompt(_) = req.content {
                        true
                    } else {
                        false
                    },
                    is_command: if let VizierRequestContent::Command(_) = req.content {
                        true
                    } else {
                        false
                    },
                    is_audio_chat: matches!(
                        &req.content,
                        VizierRequestContent::AudioChat(_, _)
                    ),
                    audio_message: match &req.content {
                        VizierRequestContent::AudioChat(att, _) => Some(att.clone()),
                        _ => None,
                    },
                    audio_transcription: match &req.content {
                        VizierRequestContent::AudioChat(_, text) => text.clone(),
                        _ => None,
                    },
                    attachments: req.attachments,
                    metadata: req.metadata,
                },
                SessionHistoryContent::Response(r) => match &r.content {
                    VizierResponseContent::Message { content: _, stats } => {
                        ContentMetadata::response {
                            stats: stats.clone(),
                            attachments: r.attachments,
                            is_audio_reply: false,
                            audio_reply: None,
                            audio_reply_text: None,
                            is_error: false,
                            error_kind: None,
                            error_message: None,
                        }
                    }
                    VizierResponseContent::AudioReply(att, text, stats) => {
                        ContentMetadata::response {
                            stats: stats.clone(),
                            attachments: r.attachments,
                            is_audio_reply: true,
                            audio_reply: Some(att.clone()),
                            audio_reply_text: text.clone(),
                            is_error: false,
                            error_kind: None,
                            error_message: None,
                        }
                    }
                    VizierResponseContent::Error { kind, message } => {
                        ContentMetadata::response {
                            stats: None,
                            attachments: r.attachments,
                            is_audio_reply: false,
                            audio_reply: None,
                            audio_reply_text: None,
                            is_error: true,
                            error_kind: Some(kind.clone()),
                            error_message: Some(message.clone()),
                        }
                    }
                    _ => ContentMetadata::response {
                        stats: None,
                        attachments: r.attachments,
                        is_audio_reply: false,
                        audio_reply: None,
                        audio_reply_text: None,
                        is_error: false,
                        error_kind: None,
                        error_message: None,
                    },
                },
                SessionHistoryContent::AssistantMessage(_) => {
                    ContentMetadata::assistant_message {}
                }
                SessionHistoryContent::ToolCall { call_id, name, .. } => {
                    ContentMetadata::tool_call { call_id, name }
                }
                SessionHistoryContent::ToolResult { call_id, .. } => {
                    ContentMetadata::tool_result { call_id }
                }
            },
        }
    }
}

#[async_trait::async_trait]
impl HistoryStorage for FileSystemStorage {
    async fn save_session_history(
        &self,
        session: VizierSession,
        content: SessionHistoryContent,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let slug = format!("{}", now);

        let history = SessionHistory {
            uid: slug.clone(),
            vizier_session: session.clone(),
            content: content.clone(),
            timestamp: Utc::now(),
            reactions: vec![],
        };

        let history_text = match &content {
            SessionHistoryContent::Request(req) => format!("{}", req.content),
            SessionHistoryContent::Response(r) => match &r.content {
                VizierResponseContent::Message { content, stats: _ } => content.clone(),
                VizierResponseContent::AudioReply(_, text, _) => text.clone().unwrap_or_default(),
                VizierResponseContent::Error { kind, message } => {
                    let kind_str = match kind {
                        ErrorKind::Completion => "completion",
                        ErrorKind::ToolTimeout => "tool_timeout",
                        ErrorKind::PromptTimeout => "prompt_timeout",
                    };
                    format!("[Error: {}] {}", kind_str, message)
                }
                _ => String::new(),
            },
            SessionHistoryContent::AssistantMessage(text) => text.clone(),
            SessionHistoryContent::ToolCall {
                name, arguments, ..
            } => {
                serde_json::to_string_pretty(&serde_json::json!({
                    "name": name,
                    "arguments": arguments
                }))
                .unwrap_or_default()
            }
            SessionHistoryContent::ToolResult { content, .. } => content.clone(),
        };

        let frontmatter = SessionHistoryFrontMatter::from(history);
        let path = PathBuf::from(format!(
            "{}/agents/{}/{}/{}/{}/{}.md",
            self.workspace,
            session.0.clone(),
            HISTORY_PATH,
            session.1.clone().to_slug(),
            session.2.clone().unwrap_or("DEFAULT".to_string()),
            slug
        ));

        let res = utils::markdown::write_markdown(&frontmatter, history_text, path.clone());

        // delete the file if the write error
        if res.is_err() {
            let _ = std::fs::remove_file(&path);
        }

        Ok(res?)
    }

    async fn list_session_history(
        &self,
        session: VizierSession,
        before: Option<chrono::DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<SessionHistory>> {
        let mut res = vec![];

        let path = format!(
            "{}/agents/{}/{}/{}/{}/*.md",
            self.workspace,
            session.0.clone(),
            HISTORY_PATH,
            session.1.to_slug(),
            session.2.clone().unwrap_or("DEFAULT".to_string()),
        );

        for entry in glob::glob(&path)? {
            let entry = entry?;

            if !entry.is_file() {
                continue;
            }

            if let Ok((frontmatter, content)) =
                utils::markdown::read_markdown::<SessionHistoryFrontMatter>(entry)
            {
                res.push(SessionHistory {
                    uid: frontmatter.uid,
                    vizier_session: frontmatter.session,
                    timestamp: frontmatter.timestamp,
                    reactions: frontmatter.reactions.unwrap_or_default(),
                    content: match frontmatter.content_metadata {
                        ContentMetadata::request {
                            user,
                            is_silent_read,
                            is_task,
                            is_chat,
                            is_prompt,
                            is_command,
                            is_audio_chat,
                            audio_message,
                            audio_transcription,
                            metadata,
                            attachments,
                        } => SessionHistoryContent::Request(VizierRequest {
                            timestamp: frontmatter.timestamp,
                            user,
                            metadata,
                            content: match (is_silent_read, is_task, is_chat, is_prompt, is_command, is_audio_chat)
                            {
                                (true, _, _, _, _, _) => VizierRequestContent::SilentRead(content),
                                (_, true, _, _, _, _) => VizierRequestContent::Task(content),
                                (_, _, true, _, _, _) => VizierRequestContent::Chat(content),
                                (_, _, _, true, _, _) => VizierRequestContent::Prompt(content),
                                (_, _, _, _, true, _) => VizierRequestContent::Command(content),
                                (_, _, _, _, _, true) => {
                                    let att = audio_message.unwrap_or_else(|| VizierAttachment {
                                        filename: "audio.webm".into(),
                                        content: VizierAttachmentContent::Local("".into()),
                                    });
                                    VizierRequestContent::AudioChat(att, audio_transcription)
                                }
                                _ => unimplemented!(),
                            },
                            platform_message_id: None,
                            attachments,
                            expect_audio_reply: None,
                        }),
                        ContentMetadata::response { stats, attachments, is_audio_reply, audio_reply, audio_reply_text, is_error, error_kind, error_message } => {
                            SessionHistoryContent::Response(VizierResponse {
                                timestamp: frontmatter.timestamp,
                                content: if is_error {
                                    VizierResponseContent::Error {
                                        kind: error_kind.unwrap_or(ErrorKind::Completion),
                                        message: error_message.unwrap_or_default(),
                                    }
                                } else if is_audio_reply {
                                    let att = audio_reply.unwrap_or_else(|| VizierAttachment {
                                        filename: "audio_reply.wav".into(),
                                        content: VizierAttachmentContent::Local("".into()),
                                    });
                                    VizierResponseContent::AudioReply(att, audio_reply_text, stats)
                                } else {
                                    VizierResponseContent::Message { content, stats }
                                },
                                attachments,
                            })
                        }
                        ContentMetadata::assistant_message {} => {
                            SessionHistoryContent::AssistantMessage(content)
                        }
                        ContentMetadata::tool_call { call_id, name } => {
                            let args: serde_json::Value = serde_json::from_str(&content).unwrap_or(serde_json::Value::Null);
                            SessionHistoryContent::ToolCall {
                                call_id,
                                name,
                                arguments: args,
                            }
                        }
                        ContentMetadata::tool_result { call_id } => {
                            SessionHistoryContent::ToolResult {
                                call_id,
                                content,
                            }
                        }
                    },
                });
            }
        }

        // Sort by timestamp descending (most recent first) for proper cursor pagination
        res.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Apply cursor filter: get items before the given datetime
        if let Some(before_dt) = before {
            res.retain(|item| item.timestamp < before_dt);
        }

        // Apply limit
        if let Some(limit_val) = limit {
            res.truncate(limit_val);
        }

        // Sort back to ascending order (oldest first) for the final result
        res.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(res)
    }

    async fn update_history_reactions(
        &self,
        uid: String,
        session: VizierSession,
        reactions: Vec<ReactionEntry>,
    ) -> Result<()> {
        let path = format!(
            "{}/agents/{}/{}/{}/{}/*.md",
            self.workspace,
            session.0.clone(),
            HISTORY_PATH,
            session.1.clone().to_slug(),
            session.2.clone().unwrap_or("DEFAULT".to_string()),
        );

        for entry in glob::glob(&path)? {
            let entry = entry?;
            if !entry.is_file() {
                continue;
            }

            let entry_path = entry.clone();
            if let Ok((frontmatter, content_text)) =
                utils::markdown::read_markdown::<SessionHistoryFrontMatter>(entry_path.clone())
            {
                if frontmatter.uid == uid {
                    let mut updated_frontmatter = frontmatter;
                    updated_frontmatter.reactions = if reactions.is_empty() {
                        None
                    } else {
                        Some(reactions.clone())
                    };

                    let history = SessionHistory {
                        uid: updated_frontmatter.uid.clone(),
                        vizier_session: updated_frontmatter.session.clone(),
                        timestamp: updated_frontmatter.timestamp,
                         content: match updated_frontmatter.content_metadata.clone() {
                            ContentMetadata::request {
                                user,
                                is_silent_read,
                                is_task,
                                is_chat,
                                is_prompt,
                                is_command,
                                is_audio_chat,
                                audio_message,
                                audio_transcription,
                                metadata,
                                attachments,
                            } => SessionHistoryContent::Request(VizierRequest {
                                timestamp: updated_frontmatter.timestamp,
                                user,
                                metadata,
                                content: match (
                                    is_silent_read, is_task, is_chat, is_prompt, is_command, is_audio_chat,
                                ) {
                                    (true, _, _, _, _, _) => {
                                        VizierRequestContent::SilentRead(content_text.clone())
                                    }
                                    (_, true, _, _, _, _) => VizierRequestContent::Task(content_text.clone()),
                                    (_, _, true, _, _, _) => VizierRequestContent::Chat(content_text.clone()),
                                    (_, _, _, true, _, _) => VizierRequestContent::Prompt(content_text.clone()),
                                    (_, _, _, _, true, _) => {
                                        VizierRequestContent::Command(content_text.clone())
                                    }
                                    (_, _, _, _, _, true) => {
                                        let att = audio_message.unwrap_or_else(|| VizierAttachment {
                                            filename: "audio.webm".into(),
                                            content: VizierAttachmentContent::Local("".into()),
                                        });
                                        VizierRequestContent::AudioChat(att, audio_transcription)
                                    }
                                    _ => unimplemented!(),
                                },
                                platform_message_id: None,
                                attachments,
                                expect_audio_reply: None,
                            }),
                            ContentMetadata::response { stats, attachments, is_audio_reply, audio_reply, audio_reply_text, is_error, error_kind, error_message } => {
                                SessionHistoryContent::Response(VizierResponse {
                                    timestamp: updated_frontmatter.timestamp,
                                    content: if is_error {
                                        VizierResponseContent::Error {
                                            kind: error_kind.unwrap_or(ErrorKind::Completion),
                                            message: error_message.unwrap_or_default(),
                                        }
                                    } else if is_audio_reply {
                                        let att = audio_reply.unwrap_or_else(|| VizierAttachment {
                                            filename: "audio_reply.wav".into(),
                                            content: VizierAttachmentContent::Local("".into()),
                                        });
                                        VizierResponseContent::AudioReply(att, audio_reply_text, stats)
                                    } else {
                                        VizierResponseContent::Message { content: content_text.clone(), stats }
                                    },
                                    attachments,
                                })
                            }
                            ContentMetadata::assistant_message {} => {
                                SessionHistoryContent::AssistantMessage(content_text.clone())
                            }
                            ContentMetadata::tool_call { call_id, name } => {
                                let args: serde_json::Value = serde_json::from_str(&content_text).unwrap_or(serde_json::Value::Null);
                                SessionHistoryContent::ToolCall {
                                    call_id,
                                    name,
                                    arguments: args,
                                }
                            }
                        ContentMetadata::tool_result { call_id } => {
                            SessionHistoryContent::ToolResult {
                                call_id,
                                content: content_text.clone(),
                            }
                        }
                    },
                        reactions: updated_frontmatter.reactions.clone().unwrap_or_default(),
                    };

                    let new_frontmatter = SessionHistoryFrontMatter::from(history);
                    let _ = utils::markdown::write_markdown(
                        &new_frontmatter,
                        content_text,
                        entry_path,
                    );
                    break;
                }
            }
        }

        Ok(())
    }

    async fn aggregate_usage(
        &self,
        agent_id: &str,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
    ) -> Result<AgentUsageStats> {
        let mut total_tokens: u64 = 0;
        let mut total_input_tokens: u64 = 0;
        let mut total_output_tokens: u64 = 0;
        let mut total_requests: u64 = 0;
        let mut total_duration_ms: u64 = 0;

        let mut by_channel_type: HashMap<String, ChannelTypeUsage> = HashMap::new();
        let mut by_day: HashMap<NaiveDate, DailyUsage> = HashMap::new();
        let mut by_day_and_channel_type: HashMap<
            NaiveDate,
            HashMap<String, ChannelTypeUsageDetail>,
        > = HashMap::new();

        let path_pattern = format!(
            "{}/agents/{}/{}/*/*/*.md",
            self.workspace, agent_id, HISTORY_PATH
        );

        for entry in glob::glob(&path_pattern)? {
            let entry = entry?;
            if !entry.is_file() {
                continue;
            }

            if let Ok((frontmatter, _)) =
                utils::markdown::read_markdown::<SessionHistoryFrontMatter>(entry.clone())
            {
                let timestamp = frontmatter.timestamp;

                if let Some(start) = start_date {
                    if timestamp < start {
                        continue;
                    }
                }
                if let Some(end) = end_date {
                    if timestamp > end {
                        continue;
                    }
                }

                if let ContentMetadata::response { stats, .. } = frontmatter.content_metadata {
                    if let Some(stats) = stats {
                        total_tokens += stats.total_tokens;
                        total_input_tokens += stats.total_input_tokens;
                        total_output_tokens += stats.total_output_tokens;
                        total_requests += 1;
                        total_duration_ms += stats.duration.as_millis() as u64;

                        let channel_slug = frontmatter.session.1.to_slug();
                        let channel_type = get_channel_type(&channel_slug);
                        let date = timestamp.date_naive();

                        let channel_entry = by_channel_type
                            .entry(channel_type.clone())
                            .or_insert_with(|| ChannelTypeUsage {
                                total_tokens: 0,
                                total_requests: 0,
                                channels: Vec::new(),
                            });
                        channel_entry.total_tokens += stats.total_tokens;
                        channel_entry.total_requests += 1;

                        let channel_id = channel_slug.clone();
                        if let Some(ch) = channel_entry
                            .channels
                            .iter_mut()
                            .find(|c| c.channel_id == channel_id)
                        {
                            ch.total_tokens += stats.total_tokens;
                            ch.total_requests += 1;
                        } else {
                            channel_entry.channels.push(ChannelUsage {
                                channel_id,
                                total_tokens: stats.total_tokens,
                                total_requests: 1,
                            });
                        }

                        let day_entry = by_day.entry(date).or_insert_with(|| DailyUsage {
                            date,
                            total_tokens: 0,
                            input_tokens: 0,
                            output_tokens: 0,
                            total_requests: 0,
                        });
                        day_entry.total_tokens += stats.total_tokens;
                        day_entry.input_tokens += stats.total_input_tokens;
                        day_entry.output_tokens += stats.total_output_tokens;
                        day_entry.total_requests += 1;

                        let day_channel_entry = by_day_and_channel_type
                            .entry(date)
                            .or_insert_with(HashMap::new);
                        let channel_detail = day_channel_entry
                            .entry(channel_type.clone())
                            .or_insert_with(|| ChannelTypeUsageDetail {
                                total_tokens: 0,
                                input_tokens: 0,
                                output_tokens: 0,
                                total_requests: 0,
                            });
                        channel_detail.total_tokens += stats.total_tokens;
                        channel_detail.input_tokens += stats.total_input_tokens;
                        channel_detail.output_tokens += stats.total_output_tokens;
                        channel_detail.total_requests += 1;
                    }
                }
            }
        }

        let mut by_day_vec: Vec<DailyUsage> = by_day.into_values().collect();
        by_day_vec.sort_by(|a, b| a.date.cmp(&b.date));

        let by_day_and_channel_type_vec: Vec<DailyChannelTypeUsage> = by_day_and_channel_type
            .into_iter()
            .map(|(date, channel_map)| DailyChannelTypeUsage {
                date,
                by_channel_type: channel_map,
            })
            .collect();
        let mut by_day_and_channel_type_vec: Vec<DailyChannelTypeUsage> =
            by_day_and_channel_type_vec;
        by_day_and_channel_type_vec.sort_by(|a, b| a.date.cmp(&b.date));

        let avg_duration_ms = if total_requests > 0 {
            total_duration_ms as f64 / total_requests as f64
        } else {
            0.0
        };

        Ok(AgentUsageStats {
            summary: UsageSummary {
                total_tokens,
                total_input_tokens,
                total_output_tokens,
                total_requests,
                avg_duration_ms,
            },
            by_channel_type,
            by_day: by_day_vec,
            by_day_and_channel_type: by_day_and_channel_type_vec,
        })
    }

    async fn list_session_by_time_window(
        &self,
        session: VizierSession,
        start_datetime: Option<DateTime<Utc>>,
        end_datetime: Option<DateTime<Utc>>,
    ) -> Result<Vec<SessionHistory>> {
        let mut res = vec![];

        let path = format!(
            "{}/agents/{}/{}/{}/{}/*.md",
            self.workspace,
            session.0.clone(),
            HISTORY_PATH,
            session.1.to_slug(),
            session.2.clone().unwrap_or("DEFAULT".to_string()),
        );

        for entry in glob::glob(&path)? {
            let entry = entry?;

            if !entry.is_file() {
                continue;
            }

            if let Ok((frontmatter, content)) =
                utils::markdown::read_markdown::<SessionHistoryFrontMatter>(entry)
            {
                let timestamp = frontmatter.timestamp;

                if let Some(start) = start_datetime {
                    if timestamp < start {
                        continue;
                    }
                }
                if let Some(end) = end_datetime {
                    if timestamp > end {
                        continue;
                    }
                }

                res.push(SessionHistory {
                    uid: frontmatter.uid,
                    vizier_session: frontmatter.session,
                    timestamp,
                    reactions: frontmatter.reactions.unwrap_or_default(),
                    content: match frontmatter.content_metadata {
                        ContentMetadata::request {
                            user,
                            is_silent_read,
                            is_task,
                            is_chat,
                            is_prompt,
                            is_command,
                            is_audio_chat,
                            audio_message,
                            audio_transcription,
                            metadata,
                            attachments,
                        } => SessionHistoryContent::Request(VizierRequest {
                            timestamp,
                            user,
                            metadata,
                            content: match (
                                is_silent_read, is_task, is_chat, is_prompt, is_command, is_audio_chat,
                            ) {
                                (true, _, _, _, _, _) => VizierRequestContent::SilentRead(content),
                                (_, true, _, _, _, _) => VizierRequestContent::Task(content),
                                (_, _, true, _, _, _) => VizierRequestContent::Chat(content),
                                (_, _, _, true, _, _) => VizierRequestContent::Prompt(content),
                                (_, _, _, _, true, _) => VizierRequestContent::Command(content),
                                (_, _, _, _, _, true) => {
                                    let att = audio_message.unwrap_or_else(|| VizierAttachment {
                                        filename: "audio.webm".into(),
                                        content: VizierAttachmentContent::Local("".into()),
                                    });
                                    VizierRequestContent::AudioChat(att, audio_transcription)
                                }
                                _ => unimplemented!(),
                            },
                            platform_message_id: None,
                            attachments,
                            expect_audio_reply: None,
                        }),
                        ContentMetadata::response { stats, attachments, is_audio_reply, audio_reply, audio_reply_text, is_error, error_kind, error_message } => {
                            SessionHistoryContent::Response(VizierResponse {
                                timestamp,
                                content: if is_error {
                                    VizierResponseContent::Error {
                                        kind: error_kind.unwrap_or(ErrorKind::Completion),
                                        message: error_message.unwrap_or_default(),
                                    }
                                } else if is_audio_reply {
                                    let att = audio_reply.unwrap_or_else(|| VizierAttachment {
                                        filename: "audio_reply.wav".into(),
                                        content: VizierAttachmentContent::Local("".into()),
                                    });
                                    VizierResponseContent::AudioReply(att, audio_reply_text, stats)
                                } else {
                                    VizierResponseContent::Message { content, stats }
                                },
                                attachments,
                            })
                        }
                        ContentMetadata::assistant_message {} => {
                            SessionHistoryContent::AssistantMessage(content)
                        }
                        ContentMetadata::tool_call { call_id, name } => {
                            let args: serde_json::Value = serde_json::from_str(&content).unwrap_or(serde_json::Value::Null);
                            SessionHistoryContent::ToolCall {
                                call_id,
                                name,
                                arguments: args,
                            }
                        }
                        ContentMetadata::tool_result { call_id } => {
                            SessionHistoryContent::ToolResult {
                                call_id,
                                content,
                            }
                        }
                    },
                });
            }
        }

        res.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(res)
    }

    async fn list_user_sessions_in_window(
        &self,
        agent_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<VizierSession>> {
        let path_pattern = format!(
            "{}/agents/{}/{}/*/*/*.md",
            self.workspace, agent_id, HISTORY_PATH
        );

        let mut seen = HashSet::new();
        let mut sessions = vec![];

        for entry in glob::glob(&path_pattern)? {
            let entry = entry?;
            if !entry.is_file() {
                continue;
            }

            if let Ok((frontmatter, _)) =
                utils::markdown::read_markdown::<SessionHistoryFrontMatter>(entry)
            {
                let timestamp = frontmatter.timestamp;
                if timestamp < start || timestamp > end {
                    continue;
                }

                let channel_slug = frontmatter.session.1.to_slug();

                // Filter out non-user channels
                if is_non_user_channel(&channel_slug) {
                    continue;
                }

                let slug = frontmatter.session.to_slug();
                if seen.insert(slug) {
                    sessions.push(frontmatter.session);
                }
            }
        }

        Ok(sessions)
    }
}

fn is_non_user_channel(channel_slug: &str) -> bool {
    channel_slug == "SYSTEM"
        || channel_slug == "SUBAGENT"
        || channel_slug.starts_with("DREAM__")
        || channel_slug.starts_with("task__")
        || channel_slug.starts_with("inter_agent__")
}

fn get_channel_type(channel_slug: &str) -> String {
    if channel_slug.starts_with("http__") {
        "http".to_string()
    } else if channel_slug.starts_with("discord__") {
        "discord".to_string()
    } else if channel_slug.starts_with("telegram__") {
        "telegram".to_string()
    } else if channel_slug.starts_with("task__") {
        "task".to_string()
    } else if channel_slug.starts_with("inter_agent__") {
        "inter_agent".to_string()
    } else if channel_slug.starts_with("heartbeat__") {
        "heartbeat".to_string()
    } else if channel_slug == "SYSTEM" {
        "system".to_string()
    } else if channel_slug == "SUBAGENT" {
        "subagent".to_string()
    } else if channel_slug.starts_with("DREAM__") {
        "dream".to_string()
    } else {
        "other".to_string()
    }
}
