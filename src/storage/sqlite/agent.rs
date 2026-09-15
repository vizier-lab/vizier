use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;

use crate::{
    schema::{
        AgentConfig, CoreRevision, CoreRevisionSummary, PaginatedCoreRevisions, RevisionDiff,
        RevisionOrigin, RevisionTrigger, RollbackResponse,
    },
    storage::{
        agent::AgentStorage,
        diff::diff_lines,
        sqlite::{
            SqliteStorage,
            core_revision::{self, CoreRevisionRow},
            memory_revision,
        },
    },
};

fn millis_to_datetime(ms: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_millis(ms).unwrap_or_else(Utc::now)
}

fn summary(row: &CoreRevisionRow, latest_seq: i64) -> CoreRevisionSummary {
    CoreRevisionSummary {
        seq: row.seq,
        actor: row.actor.clone(),
        trigger: row.trigger.clone(),
        created_at: millis_to_datetime(row.created_at),
        is_current: row.seq == latest_seq,
        size_bytes: row.content.len(),
    }
}

#[async_trait::async_trait]
impl AgentStorage for SqliteStorage {
    async fn list_agents(&self) -> Result<Vec<(String, AgentConfig)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT agent_id, data FROM agent_config")?;
        let agents = stmt
            .query_map([], |row| {
                let agent_id: String = row.get(0)?;
                let data: String = row.get(1)?;
                Ok((agent_id, data))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(agent_id, data)| {
                serde_json::from_str::<AgentConfig>(&data)
                    .ok()
                    .map(|config| (agent_id, config))
            })
            .collect();
        Ok(agents)
    }

    async fn get_agent(&self, agent_id: &str) -> Result<Option<AgentConfig>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM agent_config WHERE agent_id = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![agent_id], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        match rows.next() {
            Some(Ok(data)) => Ok(Some(serde_json::from_str(&data)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn create_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()> {
        let data = serde_json::to_string(config)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO agent_config (agent_id, data) VALUES (?1, ?2)",
            rusqlite::params![agent_id, data],
        )?;
        Ok(())
    }

    async fn update_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()> {
        let data = serde_json::to_string(config)?;
        let conn = self.conn.lock();
        let updated = conn.execute(
            "UPDATE agent_config SET data = ?1 WHERE agent_id = ?2",
            rusqlite::params![data, agent_id],
        )?;
        if updated == 0 {
            return Err(anyhow::anyhow!("Agent '{}' not found", agent_id));
        }
        Ok(())
    }

    async fn delete_agent(&self, agent_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM agent_core WHERE agent_id = ?1",
            rusqlite::params![agent_id],
        )?;
        // History dies with the agent (FR-019): both revision tables are keyed by agent_id.
        core_revision::delete_agent(&conn, agent_id)?;
        memory_revision::delete_agent(&conn, agent_id)?;
        let deleted = conn.execute(
            "DELETE FROM agent_config WHERE agent_id = ?1",
            rusqlite::params![agent_id],
        )?;
        if deleted == 0 {
            return Err(anyhow::anyhow!("Agent '{}' not found", agent_id));
        }
        Ok(())
    }

    async fn get_agent_core(&self, agent_id: &str) -> Result<Option<String>> {
        if let Some(content) = self.read_agent_core(agent_id)? {
            return Ok(Some(content));
        }
        // Legacy location: before `agent_core` was wired up, CORE lived inside the agent config
        // JSON. `VizierDependencies::migrate_agent_cores` moves it over on startup; reading it
        // here keeps a not-yet-migrated deployment intact in the meantime.
        Ok(self.get_agent(agent_id).await?.and_then(|c| c.core))
    }

    /// Upserts the CORE and appends a `core_revision` entry in one transaction, so a save and
    /// its history entry either both land or neither does (research Decision 8).
    async fn set_agent_core(
        &self,
        agent_id: &str,
        core: &str,
        origin: &RevisionOrigin,
    ) -> Result<()> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let current: Option<String> = tx
            .query_row(
                "SELECT content FROM agent_core WHERE agent_id = ?1",
                rusqlite::params![agent_id],
                |row| row.get(0),
            )
            .optional()?;
        tx.execute(
            "INSERT INTO agent_core (agent_id, content) VALUES (?1, ?2)
             ON CONFLICT(agent_id) DO UPDATE SET content = excluded.content",
            rusqlite::params![agent_id, core],
        )?;
        core_revision::record(&tx, agent_id, core, origin, current.as_deref())?;
        tx.commit()?;
        Ok(())
    }

    async fn list_core_revisions(
        &self,
        agent_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<PaginatedCoreRevisions> {
        let conn = self.conn.lock();
        // A CORE that pre-dates history gets its baseline on first listing (FR-018), so the
        // list is never empty for an agent that has a CORE.
        let current: Option<String> = conn
            .query_row(
                "SELECT content FROM agent_core WHERE agent_id = ?1",
                rusqlite::params![agent_id],
                |row| row.get(0),
            )
            .optional()?;
        core_revision::ensure_baseline(&conn, agent_id, current.as_deref())?;

        let latest_seq = core_revision::latest(&conn, agent_id)?
            .map(|r| r.seq)
            .unwrap_or(0);
        let (rows, total) = core_revision::list(&conn, agent_id, offset, limit)?;
        Ok(PaginatedCoreRevisions {
            revisions: rows.iter().map(|r| summary(r, latest_seq)).collect(),
            total,
            offset,
            limit: limit.clamp(1, core_revision::MAX_LIMIT),
        })
    }

    async fn get_core_revision(&self, agent_id: &str, seq: i64) -> Result<Option<CoreRevision>> {
        let conn = self.conn.lock();
        let Some(row) = core_revision::get(&conn, agent_id, seq)? else {
            return Ok(None);
        };
        let latest_seq = core_revision::latest(&conn, agent_id)?
            .map(|r| r.seq)
            .unwrap_or(0);
        let s = summary(&row, latest_seq);
        Ok(Some(CoreRevision {
            seq: s.seq,
            actor: s.actor,
            trigger: s.trigger,
            created_at: s.created_at,
            is_current: s.is_current,
            size_bytes: s.size_bytes,
            content: row.content,
        }))
    }

    async fn diff_core_revisions(
        &self,
        agent_id: &str,
        from: Option<i64>,
        to: i64,
    ) -> Result<RevisionDiff> {
        let conn = self.conn.lock();
        let to_row = core_revision::get(&conn, agent_id, to)?
            .ok_or_else(|| anyhow!("unknown version {to}"))?;
        let from_seq = from.unwrap_or(to - 1);
        // Diffing seq 1 "against its previous" means against nothing.
        let from_content = if from_seq < 1 {
            String::new()
        } else {
            core_revision::get(&conn, agent_id, from_seq)?
                .ok_or_else(|| anyhow!("unknown version {from_seq}"))?
                .content
        };
        let (hunks, additions, deletions) = diff_lines(&from_content, &to_row.content);
        Ok(RevisionDiff {
            from_seq: from_seq.max(0),
            to_seq: to,
            additions,
            deletions,
            hunks,
        })
    }

    async fn rollback_core(
        &self,
        agent_id: &str,
        seq: i64,
        origin: &RevisionOrigin,
    ) -> Result<RollbackResponse> {
        let (target, before) = {
            let conn = self.conn.lock();
            let target = core_revision::get(&conn, agent_id, seq)?
                .ok_or_else(|| anyhow!("unknown version {seq}"))?;
            let before = core_revision::latest(&conn, agent_id)?.map(|r| r.seq);
            (target, before)
        };
        // A rollback is just a save with rollback provenance — never a history rewrite.
        let origin = origin
            .clone()
            .with_trigger(RevisionTrigger::Rollback { restored_from: seq });
        self.set_agent_core(agent_id, &target.content, &origin).await?;
        let after = {
            let conn = self.conn.lock();
            core_revision::latest(&conn, agent_id)?.map(|r| r.seq)
        };
        let no_change = after == before;
        Ok(RollbackResponse {
            no_change,
            new_seq: if no_change { None } else { after },
            restored_from: seq,
        })
    }
}

impl SqliteStorage {
    fn read_agent_core(&self, agent_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT content FROM agent_core WHERE agent_id = ?1",
                rusqlite::params![agent_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?)
    }
}
