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

/// The only insert into `session_history`, shared by `save_session_history` and
/// `save_checkpoint` so neither can write a row without a `seq`.
///
/// `seq` is assigned by the statement rather than by a caller (FR-001). The read-then-write
/// of `MAX(seq)` is safe because every insert runs under `self.conn.lock()`, and `idx_sh_seq`
/// makes the maximum an index lookup rather than a scan.
const INSERT_HISTORY_SQL: &str = "INSERT INTO session_history \
     (uid, agent_id, channel, topic, timestamp, content_type, data, seq) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, (SELECT IFNULL(MAX(seq), 0) + 1 FROM session_history))";

/// `(data, seq)` as every ordered read selects it. `seq` lives on the column, not inside the
/// serialized `data` blob, so it has to be read back and set on the deserialized entry.
fn history_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(String, Option<i64>)> {
    Ok((row.get(0)?, row.get(1)?))
}

fn parse_history_row((data, seq): (String, Option<i64>)) -> Option<SessionHistory> {
    let mut entry = serde_json::from_str::<SessionHistory>(&data).ok()?;
    entry.seq = seq;
    Some(entry)
}

/// The sort key every in-Rust re-sort of history uses.
///
/// Keying on `timestamp` alone is the actual ordering defect: `sort_by_key` is a **stable**
/// sort, so for every tie group it preserves the order SQLite handed it — which for a
/// `DESC` query is backwards. The `ORDER BY` tie-break and this key have to change together
/// (`specs/010-webui-reasoning-display/contracts/history-api.md` H6).
fn history_order_key(entry: &SessionHistory) -> (DateTime<Utc>, Option<i64>) {
    (entry.timestamp, entry.seq)
}

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
            seq: None,
        };

        let data = serde_json::to_string(&entry)?;
        let content_type = content_type_discriminant(&entry.content);
        let conn = self.conn.lock();
        conn.execute(
            INSERT_HISTORY_SQL,
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
        before_seq: Option<i64>,
        limit: Option<usize>,
    ) -> Result<Vec<SessionHistory>> {
        let conn = self.conn.lock();
        let mut sql =
            "SELECT data, seq FROM session_history WHERE agent_id = ?1 AND channel = ?2".to_string();
        let mut param_idx = 3;

        if session.2.is_some() {
            sql.push_str(&format!(" AND topic = ?{}", param_idx));
            param_idx += 1;
        } else {
            sql.push_str(" AND topic IS NULL");
        }

        // `before` alone can split a tie group across a page boundary; with `before_seq` the
        // cursor addresses one exact entry. Passing only `before` keeps today's behaviour.
        let seq_cursor = before.is_some().then_some(before_seq).flatten();
        if before.is_some() {
            let before_param = param_idx;
            param_idx += 1;
            match seq_cursor {
                Some(_) => {
                    sql.push_str(&format!(
                        " AND (timestamp < ?{before_param} OR (timestamp = ?{before_param} AND seq < ?{}))",
                        param_idx
                    ));
                    param_idx += 1;
                }
                None => sql.push_str(&format!(" AND timestamp < ?{before_param}")),
            }
        }
        sql.push_str(" ORDER BY timestamp DESC, seq DESC");
        if limit.is_some() {
            sql.push_str(&format!(" LIMIT ?{}", param_idx));
        }

        let mut stmt = conn.prepare(&sql)?;

        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(session.0.clone()),
            Box::new(session.1.to_slug()),
        ];
        if let Some(ref topic) = session.2 {
            params.push(Box::new(topic.clone()));
        }
        if let Some(before_dt) = before {
            params.push(Box::new(before_dt.timestamp_millis()));
        }
        if let Some(seq) = seq_cursor {
            params.push(Box::new(seq));
        }
        if let Some(limit_val) = limit {
            params.push(Box::new(limit_val as i64));
        }

        let mut list: Vec<SessionHistory> = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), history_row)?
            .filter_map(|r| r.ok())
            .filter_map(parse_history_row)
            .collect();

        list.sort_by_key(history_order_key);
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
            let mut stmt = conn.prepare("SELECT data FROM session_history WHERE uid = ?1")?;
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
        let mut sql = "SELECT data FROM session_history WHERE agent_id = ?1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(agent_id.to_string())];
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

                    let channel_entry =
                        by_channel_type
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
            }
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
        let conn = self.conn.lock();
        let mut sql =
            "SELECT data, seq FROM session_history WHERE agent_id = ?1 AND channel = ?2".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(session.0.clone()),
            Box::new(session.1.to_slug()),
        ];
        let mut param_idx = 3;

        if session.2.is_some() {
            sql.push_str(&format!(" AND topic = ?{}", param_idx));
            params.push(Box::new(session.2.clone()));
            param_idx += 1;
        } else {
            sql.push_str(" AND topic IS NULL");
        }

        if let Some(start) = start_datetime {
            sql.push_str(&format!(" AND timestamp >= ?{}", param_idx));
            params.push(Box::new(start.timestamp_millis()));
            param_idx += 1;
        }
        if let Some(end) = end_datetime {
            sql.push_str(&format!(" AND timestamp <= ?{}", param_idx));
            params.push(Box::new(end.timestamp_millis()));
        }
        sql.push_str(" ORDER BY timestamp DESC, seq DESC");

        let mut stmt = conn.prepare(&sql)?;
        let mut list: Vec<SessionHistory> = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), history_row)?
            .filter_map(|r| r.ok())
            .filter_map(parse_history_row)
            .collect();

        list.sort_by_key(history_order_key);
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
            "SELECT data, seq FROM session_history WHERE agent_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3 ORDER BY timestamp DESC, seq DESC",
        )?;

        let entries: Vec<SessionHistory> = stmt
            .query_map(
                rusqlite::params![agent_id, start.timestamp_millis(), end.timestamp_millis()],
                history_row,
            )?
            .filter_map(|r| r.ok())
            .filter_map(parse_history_row)
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
        let mut cp_sql = "SELECT data, seq FROM session_history WHERE agent_id = ?1 AND channel = ?2 AND content_type = 'Checkpoint'".to_string();
        let mut cp_params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(session.0.clone()),
            Box::new(session.1.to_slug()),
        ];
        let mut cp_param_idx = 3;

        if session.2.is_some() {
            cp_sql.push_str(&format!(" AND topic = ?{}", cp_param_idx));
            cp_params.push(Box::new(session.2.clone()));
            cp_param_idx += 1;
        } else {
            cp_sql.push_str(" AND topic IS NULL");
        }

        if before.is_some() {
            cp_sql.push_str(&format!(" AND timestamp < ?{}", cp_param_idx));
            cp_params.push(Box::new(before.unwrap().timestamp_millis()));
        }
        cp_sql.push_str(" ORDER BY timestamp DESC, seq DESC LIMIT 1");

        let checkpoint = {
            let mut stmt = conn.prepare(&cp_sql)?;
            let rows: Vec<SessionHistory> = stmt
                .query_map(rusqlite::params_from_iter(cp_params.iter()), history_row)?
                .filter_map(|r| r.ok())
                .filter_map(parse_history_row)
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
        let mut hist_sql = "SELECT data, seq FROM session_history WHERE agent_id = ?1 AND channel = ?2".to_string();
        let mut hist_params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(session.0.clone()),
            Box::new(session.1.to_slug()),
        ];
        let mut param_idx = 3;

        if session.2.is_some() {
            hist_sql.push_str(&format!(" AND topic = ?{}", param_idx));
            hist_params.push(Box::new(session.2.clone()));
            param_idx += 1;
        } else {
            hist_sql.push_str(" AND topic IS NULL");
        }

        if let Some(cp_ts) = checkpoint_timestamp {
            hist_sql.push_str(&format!(" AND timestamp > ?{}", param_idx));
            hist_params.push(Box::new(cp_ts.timestamp_millis()));
            param_idx += 1;
        }
        if let Some(before_dt) = before {
            hist_sql.push_str(&format!(" AND timestamp < ?{}", param_idx));
            hist_params.push(Box::new(before_dt.timestamp_millis()));
        }
        hist_sql.push_str(" ORDER BY timestamp ASC, seq ASC");

        let mut stmt = conn.prepare(&hist_sql)?;
        let history: Vec<SessionHistory> = stmt
            .query_map(rusqlite::params_from_iter(hist_params.iter()), history_row)?
            .filter_map(|r| r.ok())
            .filter_map(parse_history_row)
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
            seq: None,
        };

        let data = serde_json::to_string(&entry)?;
        let conn = self.conn.lock();
        conn.execute(
            INSERT_HISTORY_SQL,
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;
    use rusqlite::Connection;

    use super::*;
    use crate::schema::{VizierChannelId, VizierRequest, VizierRequestContent};
    use crate::storage::document::LocalDocumentStore;

    fn setup() -> (SqliteStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_history_schema(&conn).unwrap();
        let storage = SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    fn session() -> VizierSession {
        VizierSession(
            "agent-1".to_string(),
            VizierChannelId::HTTP("someone".to_string(), "webui".to_string()),
            Some("General".to_string()),
        )
    }

    /// A `Request` carrying `text`, so an entry is identifiable in the order it comes back.
    fn request(text: &str) -> SessionHistoryContent {
        SessionHistoryContent::Request(VizierRequest {
            timestamp: Utc::now(),
            user: "someone".to_string(),
            content: VizierRequestContent::Chat(text.to_string()),
            platform_message_id: None,
            metadata: serde_json::Value::Null,
            attachments: vec![],
            expect_audio_reply: None,
        })
    }

    fn prompts(entries: &[SessionHistory]) -> Vec<String> {
        entries
            .iter()
            .filter_map(|entry| match &entry.content {
                SessionHistoryContent::Request(req) => match &req.content {
                    VizierRequestContent::Chat(text) => Some(text.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    /// Flatten every entry into the one instant they would realistically share, which is
    /// the condition `seq` exists to disambiguate. `Utc::now()` cannot be made to collide on
    /// demand, so the collision is created rather than waited for.
    ///
    /// Both copies of the timestamp have to be flattened. SQLite orders by the `timestamp`
    /// **column**, written at millisecond resolution; the in-Rust re-sort reads
    /// `SessionHistory::timestamp` out of the serialized `data` blob, which keeps
    /// nanoseconds. Collapsing only the column would leave the blob's nanoseconds to order
    /// the entries by accident and the test would pass against the defect.
    fn collapse_timestamps(storage: &SqliteStorage) {
        let conn = storage.conn.lock();
        conn.execute_batch(
            "UPDATE session_history
                SET timestamp = 1700000000000,
                    data = json_set(data, '$.timestamp', '2023-11-14T22:13:20Z');",
        )
        .unwrap();
    }

    fn seqs(storage: &SqliteStorage) -> Vec<Option<i64>> {
        let conn = storage.conn.lock();
        let mut stmt = conn
            .prepare("SELECT seq FROM session_history ORDER BY rowid")
            .unwrap();
        let rows = stmt
            .query_map([], |row| row.get::<_, Option<i64>>(0))
            .unwrap();
        rows.map(|r| r.unwrap()).collect()
    }

    /// H1, H5, SC-002.
    #[tokio::test]
    async fn seq_is_strictly_increasing_and_orders_entries_sharing_a_timestamp() {
        let (storage, _dir) = setup();
        let expected: Vec<String> = (0..25).map(|i| format!("entry {i}")).collect();

        for text in &expected {
            storage
                .save_session_history(session(), request(text))
                .await
                .unwrap();
        }

        // H1: every entry got a position, and no two share one.
        let assigned = seqs(&storage);
        let positions: Vec<i64> = assigned.iter().map(|s| s.expect("seq assigned")).collect();
        assert_eq!(positions, (1..=25).collect::<Vec<i64>>());

        // H5: with every timestamp identical, insertion order still comes back.
        collapse_timestamps(&storage);
        for _ in 0..20 {
            let list = storage
                .list_session_history(session(), None, None, None)
                .await
                .unwrap();
            assert_eq!(prompts(&list), expected);
        }
    }

    /// H7, H8, FR-007, FR-008, SC-004.
    #[tokio::test]
    async fn entries_recorded_before_seq_existed_still_load_and_do_not_disturb_the_rest() {
        let (storage, _dir) = setup();

        // Three entries as a previous build would have left them: no ordering position.
        for text in ["old a", "old b", "old c"] {
            storage
                .save_session_history(session(), request(text))
                .await
                .unwrap();
        }
        {
            let conn = storage.conn.lock();
            conn.execute_batch("UPDATE session_history SET seq = NULL;")
                .unwrap();
        }

        for text in ["new a", "new b", "new c"] {
            storage
                .save_session_history(session(), request(text))
                .await
                .unwrap();
        }
        collapse_timestamps(&storage);

        assert_eq!(
            seqs(&storage),
            vec![None, None, None, Some(1), Some(2), Some(3)],
            "a pre-existing row keeps a NULL position and a new one starts the counter over"
        );

        // H8: the read succeeds over the mix, and every entry is returned.
        let list = storage
            .list_session_history(session(), None, None, None)
            .await
            .unwrap();
        let returned = prompts(&list);
        assert_eq!(returned.len(), 6);

        // H8: the entries that do carry a position are in it, relative to each other.
        let ordered: Vec<&String> = returned
            .iter()
            .filter(|text| text.starts_with("new "))
            .collect();
        assert_eq!(ordered, vec!["new a", "new b", "new c"]);

        // H7: the NULL group is returned, in whatever order — only that it is all there.
        let mut legacy: Vec<&String> = returned
            .iter()
            .filter(|text| text.starts_with("old "))
            .collect();
        legacy.sort();
        assert_eq!(legacy, vec!["old a", "old b", "old c"]);
    }

    /// H5 on the path that feeds the agent's own replay context. It has no in-Rust re-sort
    /// at all, so its order is whatever SQLite returns — which is why the `ORDER BY`
    /// tie-break matters here even more than in the read the WebUI uses.
    #[tokio::test]
    async fn the_replay_read_is_ordered_when_entries_share_a_timestamp() {
        let (storage, _dir) = setup();
        let expected: Vec<String> = (0..25).map(|i| format!("entry {i}")).collect();

        for text in &expected {
            storage
                .save_session_history(session(), request(text))
                .await
                .unwrap();
        }
        collapse_timestamps(&storage);

        for _ in 0..20 {
            let (history, handover) = storage
                .list_session_history_until_checkpoint(session(), None)
                .await
                .unwrap();
            assert_eq!(handover, None);
            assert_eq!(prompts(&history), expected);
        }
    }
    /// The upgrade path (quickstart step 1, research D2). A database created by the previous
    /// build has `session_history` without `seq`, and `CREATE TABLE IF NOT EXISTS` will not
    /// add it — so opening it has to `ALTER`, exactly once, without disturbing what is there.
    #[test]
    fn an_existing_database_gains_the_column_without_losing_its_rows() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("vizier.db");

        // The table exactly as the previous build left it, with one row in it.
        {
            let conn = Connection::open(&db).unwrap();
            conn.execute_batch(
                "CREATE TABLE session_history (
                    uid TEXT PRIMARY KEY,
                    agent_id TEXT NOT NULL,
                    channel TEXT NOT NULL,
                    topic TEXT,
                    timestamp INTEGER NOT NULL,
                    content_type TEXT NOT NULL,
                    data TEXT NOT NULL
                );
                INSERT INTO session_history
                    VALUES ('old-1', 'a', 'http__someone__webui', 'General', 1, 'Request', '{}');",
            )
            .unwrap();
        }

        // Opening it twice: the first adds the column, the second must be a no-op rather
        // than a duplicate-column error.
        for _ in 0..2 {
            let conn = Connection::open(&db).unwrap();
            crate::storage::sqlite::init_history_schema(&conn)
                .expect("opening an upgraded database must not fail");
        }

        let conn = Connection::open(&db).unwrap();

        let seq_columns: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('session_history') WHERE name = 'seq'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(seq_columns, 1, "the column was added exactly once");

        let (uid, seq): (String, Option<i64>) = conn
            .query_row("SELECT uid, seq FROM session_history", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(uid, "old-1", "the pre-existing row is still there");
        assert_eq!(seq, None, "and keeps a NULL position — there is no backfill");

        // A row written after the upgrade gets a position, starting from one.
        conn.execute(
            INSERT_HISTORY_SQL,
            rusqlite::params!["new-1", "a", "http__someone__webui", "General", 2, "Request", "{}"],
        )
        .unwrap();
        let seq: Option<i64> = conn
            .query_row(
                "SELECT seq FROM session_history WHERE uid = 'new-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(seq, Some(1));
    }
}
