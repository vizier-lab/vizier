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
    storage::{history::HistoryStorage, sqlite::SqliteStorage},
};

fn content_type_discriminant(content: &SessionHistoryContent) -> &'static str {
    match content {
        SessionHistoryContent::Request(_) => "Request",
        SessionHistoryContent::Response(_) => "Response",
        SessionHistoryContent::AssistantMessage(_) => "AssistantMessage",
        SessionHistoryContent::ToolCall { .. } => "ToolCall",
        SessionHistoryContent::ToolResult { .. } => "ToolResult",
        SessionHistoryContent::Checkpoint(_) => "Checkpoint",
        SessionHistoryContent::Command(_) => "Command",
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

#[async_trait::async_trait]
impl HistoryStorage for SqliteStorage {
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

        let data = serde_json::to_string(&entry)?;
        let content_type = content_type_discriminant(&entry.content);
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO session_history (uid, agent_id, channel, topic, timestamp, content_type, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                uid,
                session.0,
                session.1.to_slug(),
                session.2,
                entry.timestamp.timestamp_millis(),
                content_type,
                data
            ],
        )?;
        Ok(())
    }

    async fn list_session_history(
        &self,
        session: VizierSession,
        before: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<SessionHistory>> {
        let conn = self.conn.lock();
        let mut sql = "SELECT data FROM session_history WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3".to_string();
        let mut param_idx = 4;

        if before.is_some() {
            sql.push_str(&format!(" AND timestamp < ?{}", param_idx));
            param_idx += 1;
        }
        sql.push_str(" ORDER BY timestamp DESC");
        if limit.is_some() {
            sql.push_str(&format!(" LIMIT ?{}", param_idx));
        }

        let mut stmt = conn.prepare(&sql)?;

        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(session.0.clone()),
            Box::new(session.1.to_slug()),
            Box::new(session.2.clone()),
        ];
        if let Some(before_dt) = before {
            params.push(Box::new(before_dt.timestamp_millis()));
        }
        if let Some(limit_val) = limit {
            params.push(Box::new(limit_val as i64));
        }

        let mut list: Vec<SessionHistory> = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<SessionHistory>(&data).ok())
            .collect();

        list.sort_by_key(|a| a.timestamp);
        Ok(list)
    }

    async fn update_history_reactions(
        &self,
        uid: String,
        _session: VizierSession,
        reactions: Vec<ReactionEntry>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        let data: String = {
            let mut stmt =
                conn.prepare("SELECT data FROM session_history WHERE uid = ?1")?;
            let mut rows = stmt.query_map(rusqlite::params![uid], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?;
            match rows.next() {
                Some(Ok(d)) => d,
                _ => return Ok(()),
            }
        };

        let mut entry: SessionHistory = serde_json::from_str(&data)?;
        entry.reactions = reactions;

        let new_data = serde_json::to_string(&entry)?;
        conn.execute(
            "UPDATE session_history SET data = ?1 WHERE uid = ?2",
            rusqlite::params![new_data, uid],
        )?;
        Ok(())
    }

    async fn aggregate_usage(
        &self,
        agent_id: &str,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
    ) -> Result<AgentUsageStats> {
        let conn = self.conn.lock();
        let mut sql =
            "SELECT data FROM session_history WHERE agent_id = ?1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(agent_id.to_string())];
        let mut param_idx = 2;

        if let Some(start) = start_date {
            sql.push_str(&format!(" AND timestamp >= ?{}", param_idx));
            params.push(Box::new(start.timestamp_millis()));
            param_idx += 1;
        }
        if let Some(end) = end_date {
            sql.push_str(&format!(" AND timestamp <= ?{}", param_idx));
            params.push(Box::new(end.timestamp_millis()));
        }

        let mut stmt = conn.prepare(&sql)?;
        let entries: Vec<SessionHistory> = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<SessionHistory>(&data).ok())
            .collect();

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

        for history in entries {
            if let SessionHistoryContent::Response(resp) = &history.content {
                let stats = match &resp.content {
                    VizierResponseContent::Message { stats, .. } => stats.as_ref(),
                    VizierResponseContent::AudioReply(_, _, stats) => stats.as_ref(),
                    _ => None,
                };
                if let Some(stats) = stats {
                    total_tokens += stats.total_tokens;
                    total_input_tokens += stats.total_input_tokens;
                    total_output_tokens += stats.total_output_tokens;
                    total_requests += 1;
                    total_duration_ms += stats.duration.as_millis() as u64;

                    let channel_slug = history.vizier_session.1.to_slug();
                    let channel_type = get_channel_type(&channel_slug);
                    let date = history.timestamp.date_naive();

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

                    let day_channel_entry =
                        by_day_and_channel_type.entry(date).or_default();
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

        let mut by_day_vec: Vec<DailyUsage> = by_day.into_values().collect();
        by_day_vec.sort_by_key(|a| a.date);

        let mut by_day_and_channel_type_vec: Vec<DailyChannelTypeUsage> =
            by_day_and_channel_type
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
        let conn = self.conn.lock();
        let mut sql = "SELECT data FROM session_history WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(session.0.clone()),
            Box::new(session.1.to_slug()),
            Box::new(session.2.clone()),
        ];
        let mut param_idx = 4;

        if let Some(start) = start_datetime {
            sql.push_str(&format!(" AND timestamp >= ?{}", param_idx));
            params.push(Box::new(start.timestamp_millis()));
            param_idx += 1;
        }
        if let Some(end) = end_datetime {
            sql.push_str(&format!(" AND timestamp <= ?{}", param_idx));
            params.push(Box::new(end.timestamp_millis()));
        }
        sql.push_str(" ORDER BY timestamp DESC");

        let mut stmt = conn.prepare(&sql)?;
        let mut list: Vec<SessionHistory> = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<SessionHistory>(&data).ok())
            .collect();

        list.sort_by_key(|a| a.timestamp);
        Ok(list)
    }

    async fn list_user_sessions_in_window(
        &self,
        agent_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<VizierSession>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT data FROM session_history WHERE agent_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3 ORDER BY timestamp DESC",
        )?;

        let entries: Vec<SessionHistory> = stmt
            .query_map(
                rusqlite::params![agent_id, start.timestamp_millis(), end.timestamp_millis()],
                |row| {
                    let data: String = row.get(0)?;
                    Ok(data)
                },
            )?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<SessionHistory>(&data).ok())
            .collect();

        let mut seen = HashSet::new();
        let mut sessions = vec![];

        for history in entries {
            let channel_slug = history.vizier_session.1.to_slug();
            if is_non_user_channel(&channel_slug) {
                continue;
            }
            let slug = history.vizier_session.to_slug();
            if seen.insert(slug) {
                sessions.push(history.vizier_session);
            }
        }

        Ok(sessions)
    }

    async fn list_session_history_until_checkpoint(
        &self,
        session: VizierSession,
        before: Option<DateTime<Utc>>,
    ) -> Result<(Vec<SessionHistory>, Option<String>)> {
        let conn = self.conn.lock();

        // Find latest checkpoint
        let mut cp_sql = "SELECT data FROM session_history WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3 AND content_type = 'Checkpoint'".to_string();
        let mut cp_params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(session.0.clone()),
            Box::new(session.1.to_slug()),
            Box::new(session.2.clone()),
        ];
        if let Some(before_dt) = before {
            cp_sql.push_str(" AND timestamp < ?4");
            cp_params.push(Box::new(before_dt.timestamp_millis()));
        }
        cp_sql.push_str(" ORDER BY timestamp DESC LIMIT 1");

        let checkpoint = {
            let mut stmt = conn.prepare(&cp_sql)?;
            let rows: Vec<SessionHistory> = stmt
                .query_map(rusqlite::params_from_iter(cp_params.iter()), |row| {
                    let data: String = row.get(0)?;
                    Ok(data)
                })?
                .filter_map(|r| r.ok())
                .filter_map(|data| serde_json::from_str::<SessionHistory>(&data).ok())
                .collect();
            rows.into_iter().next()
        };

        let (checkpoint_timestamp, handover) = if let Some(ref cp) = checkpoint {
            let handover = match &cp.content {
                SessionHistoryContent::Checkpoint(h) => h.clone(),
                _ => None,
            };
            (Some(cp.timestamp), handover)
        } else {
            (None, None)
        };

        // Get history after checkpoint
        let mut hist_sql = "SELECT data FROM session_history WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3".to_string();
        let mut hist_params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(session.0.clone()),
            Box::new(session.1.to_slug()),
            Box::new(session.2.clone()),
        ];
        let mut param_idx = 4;

        if let Some(cp_ts) = checkpoint_timestamp {
            hist_sql.push_str(&format!(" AND timestamp > ?{}", param_idx));
            hist_params.push(Box::new(cp_ts.timestamp_millis()));
            param_idx += 1;
        }
        if let Some(before_dt) = before {
            hist_sql.push_str(&format!(" AND timestamp < ?{}", param_idx));
            hist_params.push(Box::new(before_dt.timestamp_millis()));
        }
        hist_sql.push_str(" ORDER BY timestamp ASC");

        let mut stmt = conn.prepare(&hist_sql)?;
        let history: Vec<SessionHistory> = stmt
            .query_map(rusqlite::params_from_iter(hist_params.iter()), |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<SessionHistory>(&data).ok())
            .collect();

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

        let data = serde_json::to_string(&entry)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO session_history (uid, agent_id, channel, topic, timestamp, content_type, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                uid,
                session.0,
                session.1.to_slug(),
                session.2,
                entry.timestamp.timestamp_millis(),
                "Checkpoint",
                data
            ],
        )?;
        Ok(entry)
    }
}
