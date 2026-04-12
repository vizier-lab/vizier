use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    schema::{
        AgentUsageStats, ChannelTypeUsage, ChannelTypeUsageDetail, ChannelUsage, DailyChannelTypeUsage,
        DailyUsage, SessionHistory, SessionHistoryContent, UsageSummary, VizierChannelId,
        VizierResponseContent, VizierResponseStats, VizierSession,
    },
    storage::{history::HistoryStorage, surreal::SurrealStorage},
};

#[async_trait::async_trait]
impl HistoryStorage for SurrealStorage {
    async fn save_session_history(
        &self,
        session: VizierSession,
        content: SessionHistoryContent,
    ) -> Result<()> {
        let uuid = Uuid::new_v4();
        let _: Option<SessionHistory> = self
            .conn
            .create(("session_history", uuid.clone().to_string()))
            .content(SessionHistory {
                uid: uuid.to_string(),
                vizier_session: session.clone(),
                content,
            })
            .await?;

        Ok(())
    }

    async fn list_session_history(
        &self,
        session: VizierSession,
        before: Option<chrono::DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<SessionHistory>> {
        let query = if let Some(before_dt) = before {
            if let Some(limit_val) = limit {
                format!(
                    "SELECT * FROM session_history WHERE vizier_session == $vizier_session AND content.timestamp < {} ORDER BY content.timestamp DESC LIMIT {}",
                    before_dt.timestamp_millis(),
                    limit_val
                )
            } else {
                format!(
                    "SELECT * FROM session_history WHERE vizier_session == $vizier_session AND content.timestamp < {} ORDER BY content.timestamp DESC",
                    before_dt.timestamp_millis()
                )
            }
        } else if let Some(limit_val) = limit {
            format!(
                "SELECT * FROM session_history WHERE vizier_session == $vizier_session ORDER BY content.timestamp DESC LIMIT {}",
                limit_val
            )
        } else {
            "SELECT * FROM session_history WHERE vizier_session == $vizier_session ORDER BY content.timestamp DESC"
                .to_string()
        };

        let mut response = self
            .conn
            .query(query)
            .bind(("vizier_session", session.clone()))
            .await?;

        let mut list: Vec<SessionHistory> = response.take(0)?;

        list.sort_by(|a, b| {
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

        Ok(list)
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
        let mut by_day_and_channel_type: HashMap<NaiveDate, HashMap<String, ChannelTypeUsageDetail>> =
            HashMap::new();

        let query = if start_date.is_some() && end_date.is_some() {
            format!(
                "SELECT * FROM session_history WHERE vizier_session.0 == $agent_id AND content.timestamp >= {} AND content.timestamp <= {}",
                start_date.unwrap().timestamp_millis(),
                end_date.unwrap().timestamp_millis()
            )
        } else if start_date.is_some() {
            format!(
                "SELECT * FROM session_history WHERE vizier_session.0 == $agent_id AND content.timestamp >= {}",
                start_date.unwrap().timestamp_millis()
            )
        } else if end_date.is_some() {
            format!(
                "SELECT * FROM session_history WHERE vizier_session.0 == $agent_id AND content.timestamp <= {}",
                end_date.unwrap().timestamp_millis()
            )
        } else {
            format!(
                "SELECT * FROM session_history WHERE vizier_session.0 == $agent_id"
            )
        };

        let mut response = self
            .conn
            .query(query)
            .bind(("agent_id", agent_id.to_string()))
            .await?;

        let list: Vec<SessionHistory> = response.take(0)?;

        for history in list {
            if let SessionHistoryContent::Response(resp) = &history.content {
                if let VizierResponseContent::Message { stats, .. } = &resp.content {
                    if let Some(stats) = stats {
                        total_tokens += stats.total_tokens;
                        total_input_tokens += stats.total_input_tokens;
                        total_output_tokens += stats.total_output_tokens;
                        total_requests += 1;
                        total_duration_ms += stats.duration.as_millis() as u64;

                        let channel_slug = history.vizier_session.1.to_slug();
                        let channel_type = get_channel_type(&channel_slug);
                        let date = resp.timestamp.date_naive();

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

                        let day_entry = by_day
                            .entry(date)
                            .or_insert_with(|| DailyUsage {
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
    } else {
        "other".to_string()
    }
}
