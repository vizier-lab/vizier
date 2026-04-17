use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    schema::{
        AgentUsageStats, ChannelTypeUsage, ChannelTypeUsageDetail, ChannelUsage,
        DailyChannelTypeUsage, DailyUsage, SessionHistory, SessionHistoryContent, UsageSummary,
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
        metadata: serde_json::Value,
    },
    response {
        stats: Option<VizierResponseStats>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SessionHistoryFrontMatter {
    pub uid: String,
    pub session: VizierSession,
    pub content_metadata: ContentMetadata,
    pub timestamp: chrono::DateTime<Utc>,
}

impl From<SessionHistory> for SessionHistoryFrontMatter {
    fn from(value: SessionHistory) -> Self {
        let timestamp = match &value.content {
            SessionHistoryContent::Request(r) => r.timestamp,
            SessionHistoryContent::Response(r) => r.timestamp,
        };
        Self {
            uid: value.uid,
            timestamp,
            session: value.vizier_session,
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
                    metadata: req.metadata,
                },
                SessionHistoryContent::Response(r) => match &r.content {
                    VizierResponseContent::Message { content: _, stats } => {
                        ContentMetadata::response {
                            stats: stats.clone(),
                        }
                    }
                    _ => ContentMetadata::response { stats: None },
                },
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
        };

        let history_text = match &content {
            SessionHistoryContent::Request(req) => format!("{}", req.content),
            SessionHistoryContent::Response(r) => match &r.content {
                VizierResponseContent::Message { content, stats: _ } => content.clone(),
                _ => String::new(),
            },
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
                    content: match frontmatter.content_metadata {
                        ContentMetadata::request {
                            user,
                            is_silent_read,
                            is_task,
                            is_chat,
                            is_prompt,
                            is_command,
                            metadata,
                        } => SessionHistoryContent::Request(VizierRequest {
                            timestamp: frontmatter.timestamp,
                            user,
                            metadata,
                            content: match (is_silent_read, is_task, is_chat, is_prompt, is_command)
                            {
                                (true, _, _, _, _) => VizierRequestContent::SilentRead(content),
                                (_, true, _, _, _) => VizierRequestContent::Task(content),
                                (_, _, true, _, _) => VizierRequestContent::Chat(content),
                                (_, _, _, true, _) => VizierRequestContent::Prompt(content),
                                (_, _, _, _, true) => VizierRequestContent::Command(content),
                                _ => unimplemented!(),
                            },
                        }),
                        ContentMetadata::response { stats } => {
                            SessionHistoryContent::Response(VizierResponse {
                                timestamp: frontmatter.timestamp,
                                content: VizierResponseContent::Message { content, stats },
                            })
                        }
                    },
                });
            }
        }

        // Sort by timestamp descending (most recent first) for proper cursor pagination
        res.sort_by(|a, b| {
            let a_ts = match &a.content {
                SessionHistoryContent::Request(r) => r.timestamp,
                SessionHistoryContent::Response(r) => r.timestamp,
            };
            let b_ts = match &b.content {
                SessionHistoryContent::Request(r) => r.timestamp,
                SessionHistoryContent::Response(r) => r.timestamp,
            };
            b_ts.cmp(&a_ts)
        });

        // Apply cursor filter: get items before the given datetime
        if let Some(before_dt) = before {
            res.retain(|item| {
                let ts = match &item.content {
                    SessionHistoryContent::Request(r) => r.timestamp,
                    SessionHistoryContent::Response(r) => r.timestamp,
                };
                ts < before_dt
            });
        }

        // Apply limit
        if let Some(limit_val) = limit {
            res.truncate(limit_val);
        }

        // Sort back to ascending order (oldest first) for the final result
        res.sort_by(|a, b| {
            let a_ts = match &a.content {
                SessionHistoryContent::Request(r) => r.timestamp,
                SessionHistoryContent::Response(r) => r.timestamp,
            };
            let b_ts = match &b.content {
                SessionHistoryContent::Request(r) => r.timestamp,
                SessionHistoryContent::Response(r) => r.timestamp,
            };
            a_ts.cmp(&b_ts)
        });

        Ok(res)
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
                    content: match frontmatter.content_metadata {
                        ContentMetadata::request {
                            user,
                            is_silent_read,
                            is_task,
                            is_chat,
                            is_prompt,
                            is_command,
                            metadata,
                        } => SessionHistoryContent::Request(VizierRequest {
                            timestamp,
                            user,
                            metadata,
                            content: match (is_silent_read, is_task, is_chat, is_prompt, is_command)
                            {
                                (true, _, _, _, _) => VizierRequestContent::SilentRead(content),
                                (_, true, _, _, _) => VizierRequestContent::Task(content),
                                (_, _, true, _, _) => VizierRequestContent::Chat(content),
                                (_, _, _, true, _) => VizierRequestContent::Prompt(content),
                                (_, _, _, _, true) => VizierRequestContent::Command(content),
                                _ => unimplemented!(),
                            },
                        }),
                        ContentMetadata::response { stats } => {
                            SessionHistoryContent::Response(VizierResponse {
                                timestamp,
                                content: VizierResponseContent::Message { content, stats },
                            })
                        }
                    },
                });
            }
        }

        res.sort_by(|a, b| {
            let a_ts = match &a.content {
                SessionHistoryContent::Request(r) => r.timestamp,
                SessionHistoryContent::Response(r) => r.timestamp,
            };
            let b_ts = match &b.content {
                SessionHistoryContent::Request(r) => r.timestamp,
                SessionHistoryContent::Response(r) => r.timestamp,
            };
            a_ts.cmp(&b_ts)
        });

        Ok(res)
    }
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
