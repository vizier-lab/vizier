use anyhow::Result;

use crate::schema::{
    AgentId, DreamStage,
    dream_journal::{ChangeKind, DreamEntryStub, DreamJournalEntry, DreamRevision, change_kind},
};
use crate::storage::{dream_journal::DreamJournalStorage, sqlite::SqliteStorage};

#[async_trait::async_trait]
impl DreamJournalStorage for SqliteStorage {
    async fn save_dream_entry(&self, entry: DreamJournalEntry) -> Result<()> {
        let data = serde_json::to_string(&entry)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO dream_journal (id, agent_id, dream_cycle_id, stage, timestamp, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                entry.id,
                entry.agent_id,
                entry.dream_cycle_id,
                serde_json::to_string(&entry.stage)?.trim_matches('"'),
                entry.timestamp.timestamp_millis(),
                data
            ],
        )?;
        Ok(())
    }

    async fn list_dream_entries(
        &self,
        agent_id: AgentId,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DreamJournalEntry>> {
        let conn = self.conn.lock();
        // SQLite rejects OFFSET without LIMIT; a negative LIMIT means no limit.
        let limit_val = limit.map_or(-1, |l| l as i64);
        let offset_val = offset.unwrap_or(0) as i64;

        let mut stmt = conn.prepare(
            "SELECT data FROM dream_journal WHERE agent_id = ?1 ORDER BY timestamp DESC LIMIT ?2 OFFSET ?3",
        )?;
        let entries = stmt
            .query_map(rusqlite::params![agent_id, limit_val, offset_val], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<DreamJournalEntry>(&data).ok())
            .collect();
        Ok(entries)
    }

    async fn get_dream_entry(
        &self,
        _agent_id: AgentId,
        entry_id: String,
    ) -> Result<Option<DreamJournalEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM dream_journal WHERE id = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![entry_id], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn get_latest_dream_entry(
        &self,
        agent_id: AgentId,
        stage: DreamStage,
    ) -> Result<Option<DreamJournalEntry>> {
        let stage_str = serde_json::to_string(&stage)?.trim_matches('"').to_string();
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT data FROM dream_journal WHERE agent_id = ?1 AND stage = ?2 ORDER BY timestamp DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![agent_id, stage_str], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn list_dream_entries_by_cycle(
        &self,
        agent_id: AgentId,
        cycle_id: &str,
        stage: Option<DreamStage>,
    ) -> Result<Vec<DreamJournalEntry>> {
        let conn = self.conn.lock();
        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match stage {
            Some(s) => {
                let stage_str = serde_json::to_string(&s)?.trim_matches('"').to_string();
                (
                    "SELECT data FROM dream_journal WHERE agent_id = ?1 AND dream_cycle_id = ?2 AND stage = ?3 ORDER BY timestamp ASC".to_string(),
                    vec![
                        Box::new(agent_id) as Box<dyn rusqlite::types::ToSql>,
                        Box::new(cycle_id.to_string()),
                        Box::new(stage_str),
                    ],
                )
            }
            None => (
                "SELECT data FROM dream_journal WHERE agent_id = ?1 AND dream_cycle_id = ?2 ORDER BY timestamp ASC".to_string(),
                vec![
                    Box::new(agent_id) as Box<dyn rusqlite::types::ToSql>,
                    Box::new(cycle_id.to_string()),
                ],
            ),
        };

        let mut stmt = conn.prepare(&sql)?;
        let entries = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<DreamJournalEntry>(&data).ok())
            .collect();
        Ok(entries)
    }

    async fn list_dream_entry_stubs(
        &self,
        agent_id: AgentId,
        since: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<DreamEntryStub>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT dream_cycle_id, stage, timestamp,
                    json_extract(data, '$.source_sessions'),
                    json_extract(data, '$.provider_used'), json_extract(data, '$.model_used')
             FROM dream_journal
             WHERE agent_id = ?1 AND dream_cycle_id IS NOT NULL AND timestamp >= ?2
             ORDER BY timestamp ASC",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![agent_id, since.timestamp_millis()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(cycle_id, stage, ts, sources, provider, model)| {
                Some(DreamEntryStub {
                    cycle_id,
                    stage: serde_json::from_value(serde_json::Value::String(stage)).ok()?,
                    timestamp: chrono::DateTime::from_timestamp_millis(ts)?,
                    source_sessions: sources
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                    provider,
                    model,
                })
            })
            .collect();
        Ok(rows)
    }

    async fn list_dream_revisions(
        &self,
        agent_id: AgentId,
        since: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<DreamRevision>> {
        let conn = self.conn.lock();
        let since = since.timestamp_millis();
        let mut out = Vec::new();

        // Memory: `previous_deleted` is NULL when the document has no earlier revision.
        let mut stmt = conn.prepare(
            "SELECT r.bundle, r.path, r.seq, r.deleted, r.created_at, r.dream_cycle_id,
                    (SELECT p.deleted FROM memory_revision p
                      WHERE p.agent_id = r.agent_id AND p.bundle = r.bundle AND p.path = r.path AND p.seq < r.seq
                      ORDER BY p.seq DESC LIMIT 1)
             FROM memory_revision r
             WHERE r.agent_id = ?1 AND r.trigger = 'dream' AND r.created_at >= ?2
             ORDER BY r.created_at ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![agent_id, since], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)? != 0,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<i64>>(6)?.map(|d| d != 0),
            ))
        })?;
        for (bundle, path, seq, deleted, at, cycle_id, previous_deleted) in rows.filter_map(|r| r.ok()) {
            let Some(at) = chrono::DateTime::from_timestamp_millis(at) else { continue };
            out.push(DreamRevision {
                kind: change_kind(false, deleted, previous_deleted),
                bundle: Some(bundle),
                path: Some(path),
                seq,
                at,
                cycle_id,
            });
        }

        let mut stmt = conn.prepare(
            "SELECT seq, created_at, dream_cycle_id FROM core_revision
             WHERE agent_id = ?1 AND trigger = 'dream' AND created_at >= ?2 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![agent_id, since], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, Option<String>>(2)?))
        })?;
        for (seq, at, cycle_id) in rows.filter_map(|r| r.ok()) {
            let Some(at) = chrono::DateTime::from_timestamp_millis(at) else { continue };
            out.push(DreamRevision { kind: ChangeKind::Core, bundle: None, path: None, seq, at, cycle_id });
        }

        out.sort_by_key(|r| r.at);
        Ok(out)
    }
}
