use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::OptionalExtension;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::{
    schema::{
        ClientCallOutcome, ReactionEntry, SessionHistory, SessionHistoryContent,
        VizierAttachmentContent, VizierResponseContent, VizierSession,
    },
    storage::{
        history::{HistoryStorage, is_non_user_channel},
        sqlite::SqliteStorage,
    },
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

pub(super) fn parse_history_row((data, seq): (String, Option<i64>)) -> Option<SessionHistory> {
    let mut entry = serde_json::from_str::<SessionHistory>(&data).ok()?;
    entry.seq = seq;
    Some(entry)
}

/// Fill `available` on every artifact attachment of a page (`specs/035-add-artifacts` §6).
/// Read-time only; the stored JSON never carries it. One indexed artifact lookup per
/// attachment, on the connection the history read already holds.
pub(super) fn fill_artifact_availability(
    conn: &rusqlite::Connection,
    entries: &mut [SessionHistory],
) -> rusqlite::Result<()> {
    for entry in entries.iter_mut() {
        for attachment in entry.content.attachments_mut() {
            if let VizierAttachmentContent::Artifact {
                agent_id,
                artifact_id,
                available,
            } = &mut attachment.content
            {
                let exists = conn
                    .query_row(
                        "SELECT content_file FROM artifact WHERE agent_id = ?1 AND id = ?2",
                        rusqlite::params![agent_id.as_str(), artifact_id.as_str()],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?
                    .is_some();
                *available = Some(exists);
            }
        }
    }
    Ok(())
}

/// Attach each entry's reactions from `message_reaction`, which is their only home: the
/// `data` blob never carries them (`specs/013-reaction-awareness/research.md` Decision 1).
/// One query per 500 entries, so a page of history costs one extra indexed lookup.
pub(super) fn fill_reactions(
    conn: &rusqlite::Connection,
    entries: &mut [SessionHistory],
) -> rusqlite::Result<()> {
    const CHUNK: usize = 500;
    if entries.is_empty() {
        return Ok(());
    }

    let mut by_uid: HashMap<String, Vec<ReactionEntry>> = HashMap::new();
    for chunk in entries.chunks(CHUNK) {
        let placeholders = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT history_uid, reactor_id, reactor_name, emoji FROM message_reaction \
             WHERE history_uid IN ({placeholders}) ORDER BY added_at, rowid"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(chunk.iter().map(|entry| entry.uid.as_str())),
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ReactionEntry {
                        user_id: row.get(1)?,
                        user_name: row.get(2)?,
                        emoji: row.get(3)?,
                    },
                ))
            },
        )?;
        for row in rows {
            let (uid, reaction) = row?;
            by_uid.entry(uid).or_default().push(reaction);
        }
    }

    for entry in entries.iter_mut() {
        entry.reactions = by_uid.remove(&entry.uid).unwrap_or_default();
    }
    Ok(())
}

/// Fill each `ClientToolCall` entry's `outcome` and `state` from `client_tool_call`, their
/// only home (`specs/019-client-tools` research R8). Costs nothing for a page without calls.
pub(super) fn fill_client_calls(
    conn: &rusqlite::Connection,
    entries: &mut [SessionHistory],
) -> rusqlite::Result<()> {
    const CHUNK: usize = 500;
    let uids: Vec<String> = entries
        .iter()
        .filter(|entry| matches!(entry.content, SessionHistoryContent::ClientToolCall { .. }))
        .map(|entry| entry.uid.clone())
        .collect();
    if uids.is_empty() {
        return Ok(());
    }

    let mut by_uid: HashMap<String, (String, Option<String>, Option<String>)> = HashMap::new();
    for chunk in uids.chunks(CHUNK) {
        let placeholders = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT history_uid, status, detail, state FROM client_tool_call \
             WHERE history_uid IN ({placeholders})"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(chunk.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                (row.get(1)?, row.get(2)?, row.get(3)?),
            ))
        })?;
        for row in rows {
            let (uid, columns) = row?;
            by_uid.insert(uid, columns);
        }
    }

    let parse = |text: Option<String>| text.and_then(|text| serde_json::from_str(&text).ok());
    for entry in entries.iter_mut() {
        if let SessionHistoryContent::ClientToolCall { outcome, state, .. } = &mut entry.content {
            match by_uid.remove(&entry.uid) {
                Some((status, detail, saved)) => {
                    *outcome = Some(ClientCallOutcome::from_columns(&status, parse(detail)));
                    *state = parse(saved);
                }
                // A call whose row was never written (its save failed) has no outcome to show.
                None => {
                    *outcome = None;
                    *state = None;
                }
            }
        }
    }
    Ok(())
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
        SessionHistoryContent::ClientToolCall { .. } => "ClientToolCall",
        SessionHistoryContent::Recall { .. } => "Recall",
    }
}

#[async_trait::async_trait]
impl HistoryStorage for SqliteStorage {
    async fn last_activity_by_agent(
        &self,
    ) -> anyhow::Result<std::collections::HashMap<String, DateTime<Utc>>> {
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare("SELECT agent_id, MAX(timestamp) FROM session_history GROUP BY agent_id")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(id, ms)| DateTime::from_timestamp_millis(ms).map(|t| (id, t)))
            .collect();
        Ok(rows)
    }

    async fn save_session_history(
        &self,
        session: VizierSession,
        content: SessionHistoryContent,
    ) -> Result<String> {
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
        let session_slug = session.to_slug();
        let references: Vec<(String, String)> = entry
            .content
            .attachments()
            .into_iter()
            .filter_map(|attachment| match &attachment.content {
                VizierAttachmentContent::Artifact {
                    agent_id,
                    artifact_id,
                    ..
                } => Some((agent_id.clone(), artifact_id.clone())),
                _ => None,
            })
            .collect();

        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
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
        for (agent_id, artifact_id) in references {
            tx.execute(
                "INSERT OR REPLACE INTO artifact_reference \
                 (artifact_id, agent_id, kind, target, session_slug, history_uid, channel, topic, bundle, path, label, created_at) \
                 VALUES (?1, ?2, 'message', ?3, ?4, ?5, ?6, ?7, NULL, NULL, ?8, ?9)",
                rusqlite::params![
                    artifact_id,
                    agent_id,
                    format!("message:{session_slug}:{uid}"),
                    session_slug,
                    uid,
                    session.1.to_slug(),
                    session.2,
                    session.1.to_slug(),
                    entry.timestamp.timestamp_millis(),
                ],
            )?;
        }
        tx.commit()?;
        Ok(uid)
    }

    async fn list_session_history(
        &self,
        session: VizierSession,
        before: Option<DateTime<Utc>>,
        before_seq: Option<i64>,
        limit: Option<usize>,
    ) -> Result<Vec<SessionHistory>> {
        let conn = self.conn.lock();
        let mut sql = "SELECT data, seq FROM session_history WHERE agent_id = ?1 AND channel = ?2"
            .to_string();
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

        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(session.0.clone()), Box::new(session.1.to_slug())];
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
        fill_reactions(&conn, &mut list)?;
        fill_artifact_availability(&conn, &mut list)?;
        fill_client_calls(&conn, &mut list)?;
        Ok(list)
    }

    async fn list_session_by_time_window(
        &self,
        session: VizierSession,
        start_datetime: Option<DateTime<Utc>>,
        end_datetime: Option<DateTime<Utc>>,
    ) -> Result<Vec<SessionHistory>> {
        let conn = self.conn.lock();
        let mut sql = "SELECT data, seq FROM session_history WHERE agent_id = ?1 AND channel = ?2"
            .to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(session.0.clone()), Box::new(session.1.to_slug())];
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
        fill_reactions(&conn, &mut list)?;
        fill_artifact_availability(&conn, &mut list)?;
        fill_client_calls(&conn, &mut list)?;
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
        let mut cp_params: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(session.0.clone()), Box::new(session.1.to_slug())];
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
        let mut hist_sql =
            "SELECT data, seq FROM session_history WHERE agent_id = ?1 AND channel = ?2"
                .to_string();
        let mut hist_params: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(session.0.clone()), Box::new(session.1.to_slug())];
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
        let mut history: Vec<SessionHistory> = stmt
            .query_map(rusqlite::params_from_iter(hist_params.iter()), history_row)?
            .filter_map(|r| r.ok())
            .filter_map(parse_history_row)
            .collect();
        drop(stmt);
        fill_reactions(&conn, &mut history)?;
        fill_artifact_availability(&conn, &mut history)?;
        fill_client_calls(&conn, &mut history)?;

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

    async fn count_session_requests(&self, session: VizierSession) -> Result<usize> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM session_history \
             WHERE agent_id = ?1 AND channel = ?2 AND topic IS ?3 AND content_type = 'Request'",
            rusqlite::params![session.0, session.1.to_slug(), session.2],
            |row| row.get(0),
        )?;
        Ok(count as usize)
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
        crate::storage::sqlite::init_artifact_schema(&conn).unwrap();
        crate::storage::sqlite::init_history_schema(&conn).unwrap();
        crate::storage::sqlite::init_reaction_schema(&conn).unwrap();
        crate::storage::sqlite::init_client_call_schema(&conn).unwrap();
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
            scheduled_task: None,
            background_depth: 0,
            ..Default::default()
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
        assert_eq!(
            seq, None,
            "and keeps a NULL position — there is no backfill"
        );

        // A row written after the upgrade gets a position, starting from one.
        conn.execute(
            INSERT_HISTORY_SQL,
            rusqlite::params![
                "new-1",
                "a",
                "http__someone__webui",
                "General",
                2,
                "Request",
                "{}"
            ],
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

    /// The session title is generated once a conversation has enough requests; the count
    /// comes from storage so it survives a restart (#49).
    #[tokio::test]
    async fn count_session_requests_counts_only_that_sessions_requests() {
        let (storage, _dir) = setup();
        let other_topic = VizierSession(session().0, session().1, Some("Other".to_string()));
        let no_topic = VizierSession(session().0, session().1, None);

        for text in ["one", "two", "three"] {
            storage
                .save_session_history(session(), request(text))
                .await
                .unwrap();
        }
        storage
            .save_session_history(
                session(),
                SessionHistoryContent::Command("abort".to_string()),
            )
            .await
            .unwrap();
        storage
            .save_checkpoint(session(), Some("handover".to_string()))
            .await
            .unwrap();
        storage
            .save_session_history(session(), request("after checkpoint"))
            .await
            .unwrap();
        storage
            .save_session_history(other_topic.clone(), request("elsewhere"))
            .await
            .unwrap();
        storage
            .save_session_history(no_topic.clone(), request("no topic"))
            .await
            .unwrap();

        assert_eq!(storage.count_session_requests(session()).await.unwrap(), 4);
        assert_eq!(
            storage.count_session_requests(other_topic).await.unwrap(),
            1
        );
        assert_eq!(storage.count_session_requests(no_topic).await.unwrap(), 1);
    }

    /// A client tool call reads back with the outcome and state recorded after it was saved
    /// (specs/019-client-tools R8), and neither is ever written into the blob.
    #[tokio::test]
    async fn a_client_tool_call_reads_back_with_its_outcome_and_state() {
        use crate::storage::client_call::ClientCallStorage;

        let (storage, _dir) = setup();
        let uid = storage
            .save_session_history(
                session(),
                SessionHistoryContent::ClientToolCall {
                    name: "palette_pick_color".into(),
                    category: "palette".into(),
                    tool: "pick_color".into(),
                    arguments: serde_json::json!({ "prompt": "header" }),
                    outcome: None,
                    state: None,
                },
            )
            .await
            .unwrap();
        storage.open_client_call(&uid, &session()).await.unwrap();

        async fn read(
            storage: &SqliteStorage,
            uid: &str,
        ) -> (Option<ClientCallOutcome>, Option<serde_json::Value>) {
            let history = storage
                .list_session_history(session(), None, None, None)
                .await
                .unwrap();
            match history.into_iter().find(|e| e.uid == uid).unwrap().content {
                SessionHistoryContent::ClientToolCall { outcome, state, .. } => (outcome, state),
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(
            read(&storage, &uid).await,
            (Some(ClientCallOutcome::Pending), None)
        );

        let ok = ClientCallOutcome::Ok {
            result: serde_json::json!({ "hex": "#3366ff" }),
        };
        storage.finish_client_call(&uid, &ok).await.unwrap();
        storage
            .save_client_call_state(&uid, &serde_json::json!({ "n": 3 }))
            .await
            .unwrap();
        assert_eq!(
            read(&storage, &uid).await,
            (Some(ok), Some(serde_json::json!({ "n": 3 })))
        );

        let blob: String = storage
            .conn
            .lock()
            .query_row(
                "SELECT data FROM session_history WHERE uid = ?1",
                [&uid],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            !blob.contains("outcome") && !blob.contains("\"state\""),
            "{blob}"
        );
    }
}
