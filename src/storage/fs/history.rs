use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::{
    schema::{
        AgentUsageStats, ChannelTypeUsage, ChannelTypeUsageDetail, ChannelUsage,
        DailyChannelTypeUsage, DailyUsage, ReactionEntry, SessionHistory, SessionHistoryContent,
        UsageSummary, VizierResponseContent, VizierSession,
    },
    storage::{
        fs::{FileSystemStorage, HISTORY_PATH},
        history::HistoryStorage,
    },
};

fn topic_dir(workspace: &str, session: &VizierSession) -> PathBuf {
    PathBuf::from(format!(
        "{}/agents/{}/{}/{}/{}",
        workspace,
        session.0,
        HISTORY_PATH,
        session.1.to_slug(),
        session.2.clone().unwrap_or("DEFAULT".to_string()),
    ))
}

fn entry_path(workspace: &str, session: &VizierSession, uid: &str) -> PathBuf {
    topic_dir(workspace, session).join(format!("{}.json", uid))
}

fn read_entry(path: &PathBuf) -> Option<SessionHistory> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

#[async_trait::async_trait]
impl HistoryStorage for FileSystemStorage {
    async fn save_session_history(
        &self,
        session: VizierSession,
        content: SessionHistoryContent,
    ) -> Result<()> {
        let uid = Uuid::new_v4().to_string();
        let entry = SessionHistory {
            uid: uid.clone(),
            vizier_session: session.clone(),
            content,
            timestamp: Utc::now(),
            reactions: vec![],
        };

        let path = entry_path(&self.workspace, &session, &uid);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let json = serde_json::to_string_pretty(&entry)?;
        tokio::fs::write(&path, json).await?;

        Ok(())
    }

    async fn list_session_history(
        &self,
        session: VizierSession,
        before: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<SessionHistory>> {
        let pattern = format!("{}/*.json", topic_dir(&self.workspace, &session).display());
        let mut res: Vec<SessionHistory> = glob::glob(&pattern)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.is_file())
            .filter_map(|path| read_entry(&path))
            .collect();

        res.sort_by_key(|b| std::cmp::Reverse(b.timestamp));

        if let Some(before_dt) = before {
            res.retain(|item| item.timestamp < before_dt);
        }

        if let Some(limit_val) = limit {
            res.truncate(limit_val);
        }

        res.sort_by_key(|a| a.timestamp);

        Ok(res)
    }

    async fn update_history_reactions(
        &self,
        uid: String,
        session: VizierSession,
        reactions: Vec<ReactionEntry>,
    ) -> Result<()> {
        let path = entry_path(&self.workspace, &session, &uid);
        let raw = tokio::fs::read_to_string(&path).await?;
        let mut entry: SessionHistory = serde_json::from_str(&raw)?;
        entry.reactions = reactions;
        let json = serde_json::to_string_pretty(&entry)?;
        tokio::fs::write(&path, json).await?;
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
            "{}/agents/{}/{}/*/*/*.json",
            self.workspace, agent_id, HISTORY_PATH
        );

        for entry in glob::glob(&path_pattern)?
            .filter_map(|e| e.ok())
            .filter(|e| e.is_file())
        {
            let Some(entry) = read_entry(&entry) else {
                continue;
            };

            let timestamp = entry.timestamp;
            if let Some(start) = start_date
                && timestamp < start
            {
                continue;
            }
            if let Some(end) = end_date
                && timestamp > end
            {
                continue;
            }

            let stats = match &entry.content {
                SessionHistoryContent::Response(r) => match &r.content {
                    VizierResponseContent::Message { stats, .. } => stats.as_ref(),
                    VizierResponseContent::AudioReply(_, _, stats) => stats.as_ref(),
                    _ => None,
                },
                _ => None,
            };

            let Some(stats) = stats else {
                continue;
            };

            total_tokens += stats.total_tokens;
            total_input_tokens += stats.total_input_tokens;
            total_output_tokens += stats.total_output_tokens;
            total_requests += 1;
            total_duration_ms += stats.duration.as_millis() as u64;

            let channel_slug = entry.vizier_session.1.to_slug();
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

            let day_channel_entry = by_day_and_channel_type.entry(date).or_default();
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

        let mut by_day_vec: Vec<DailyUsage> = by_day.into_values().collect();
        by_day_vec.sort_by_key(|a| a.date);

        let mut by_day_and_channel_type_vec: Vec<DailyChannelTypeUsage> = by_day_and_channel_type
            .into_iter()
            .map(|(date, channel_map)| DailyChannelTypeUsage {
                date,
                by_channel_type: channel_map,
            })
            .collect();
        by_day_and_channel_type_vec.sort_by_key(|a| a.date);

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
        let pattern = format!("{}/*.json", topic_dir(&self.workspace, &session).display());
        let mut res: Vec<SessionHistory> = glob::glob(&pattern)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.is_file())
            .filter_map(|path| read_entry(&path))
            .filter(|entry| {
                if let Some(start) = start_datetime
                    && entry.timestamp < start
                {
                    return false;
                }
                if let Some(end) = end_datetime
                    && entry.timestamp > end
                {
                    return false;
                }
                true
            })
            .collect();

        res.sort_by_key(|a| a.timestamp);

        Ok(res)
    }

    async fn list_user_sessions_in_window(
        &self,
        agent_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<VizierSession>> {
        let path_pattern = format!(
            "{}/agents/{}/{}/*/*/*.json",
            self.workspace, agent_id, HISTORY_PATH
        );

        let mut seen = HashSet::new();
        let mut sessions = vec![];

        for entry in glob::glob(&path_pattern)?
            .filter_map(|e| e.ok())
            .filter(|e| e.is_file())
        {
            let Some(entry) = read_entry(&entry) else {
                continue;
            };

            if entry.timestamp < start || entry.timestamp > end {
                continue;
            }

            let channel_slug = entry.vizier_session.1.to_slug();

            if is_non_user_channel(&channel_slug) {
                continue;
            }

            let slug = entry.vizier_session.to_slug();
            if seen.insert(slug) {
                sessions.push(entry.vizier_session);
            }
        }

        Ok(sessions)
    }

    async fn list_session_history_until_checkpoint(
        &self,
        session: VizierSession,
        before: Option<DateTime<Utc>>,
    ) -> Result<(Vec<SessionHistory>, Option<String>)> {
        let pattern = format!("{}/*.json", topic_dir(&self.workspace, &session).display());
        let mut all_entries: Vec<SessionHistory> = glob::glob(&pattern)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.is_file())
            .filter_map(|path| read_entry(&path))
            .collect();

        all_entries.sort_by_key(|b| std::cmp::Reverse(b.timestamp));

        // Filter by timestamp if before is specified
        if let Some(before_dt) = before {
            all_entries.retain(|item| item.timestamp < before_dt);
        }

        // Find the latest checkpoint
        let checkpoint = all_entries
            .iter()
            .find(|e| matches!(e.content, SessionHistoryContent::Checkpoint(_)));

        let (checkpoint_timestamp, handover) = if let Some(cp) = checkpoint {
            let handover = match &cp.content {
                SessionHistoryContent::Checkpoint(h) => h.clone(),
                _ => None,
            };
            (Some(cp.timestamp), handover)
        } else {
            (None, None)
        };

        // Get all history after the checkpoint
        let mut history: Vec<SessionHistory> = all_entries
            .into_iter()
            .filter(|e| {
                if let Some(cp_ts) = checkpoint_timestamp {
                    e.timestamp > cp_ts
                } else {
                    true
                }
            })
            .collect();

        history.sort_by_key(|a| a.timestamp);

        Ok((history, handover))
    }

    async fn save_checkpoint(
        &self,
        session: VizierSession,
        handover: Option<String>,
    ) -> Result<SessionHistory> {
        let uid = Uuid::new_v4().to_string();
        let entry = SessionHistory {
            uid: uid.clone(),
            vizier_session: session.clone(),
            content: SessionHistoryContent::Checkpoint(handover),
            timestamp: Utc::now(),
            reactions: vec![],
        };

        let path = entry_path(&self.workspace, &session, &uid);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let json = serde_json::to_string_pretty(&entry)?;
        tokio::fs::write(&path, json).await?;

        Ok(entry)
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
