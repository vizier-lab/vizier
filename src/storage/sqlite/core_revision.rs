//! CORE.md version history — the `core_revision` table.
//!
//! Synchronous helpers over a `&rusqlite::Connection`, called from
//! `SqliteStorage::set_agent_core` (inside its transaction) and the `AgentStorage` history
//! methods. Memory documents have their own, separate module (`memory_revision`); the two share
//! only the value types in `schema::revision`.

use anyhow::{Result, anyhow};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::schema::{RevisionActor, RevisionOrigin, RevisionTrigger};

pub const DEFAULT_LIMIT: usize = 50;
pub const MAX_LIMIT: usize = 200;

#[derive(Debug, Clone)]
pub struct CoreRevisionRow {
    pub seq: i64,
    pub content: String,
    pub actor: RevisionActor,
    pub trigger: RevisionTrigger,
    pub restored_from: Option<i64>,
    pub created_at: i64,
}

fn clamp_limit(limit: usize) -> usize {
    limit.clamp(1, MAX_LIMIT)
}

const SELECT_COLUMNS: &str =
    "seq, content, actor_kind, actor_id, actor_name, trigger, restored_from, created_at";

fn map_row(row: &Row) -> rusqlite::Result<CoreRevisionRow> {
    let actor_kind: String = row.get(2)?;
    let trigger: String = row.get(5)?;
    let restored_from: Option<i64> = row.get(6)?;
    Ok(CoreRevisionRow {
        seq: row.get(0)?,
        content: row.get(1)?,
        actor: RevisionActor::from_columns(&actor_kind, row.get(3)?, row.get(4)?),
        trigger: RevisionTrigger::from_columns(&trigger, restored_from),
        restored_from,
        created_at: row.get(7)?,
    })
}

pub(crate) fn latest(conn: &Connection, agent_id: &str) -> Result<Option<CoreRevisionRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM core_revision WHERE agent_id = ?1 ORDER BY seq DESC LIMIT 1"
    ))?;
    Ok(stmt.query_row(params![agent_id], map_row).optional()?)
}

pub(crate) fn get(conn: &Connection, agent_id: &str, seq: i64) -> Result<Option<CoreRevisionRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM core_revision WHERE agent_id = ?1 AND seq = ?2"
    ))?;
    Ok(stmt.query_row(params![agent_id, seq], map_row).optional()?)
}

/// Newest-first page plus the total row count for the agent.
pub(crate) fn list(
    conn: &Connection,
    agent_id: &str,
    offset: usize,
    limit: usize,
) -> Result<(Vec<CoreRevisionRow>, usize)> {
    let limit = clamp_limit(limit);
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM core_revision WHERE agent_id = ?1",
        params![agent_id],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM core_revision WHERE agent_id = ?1
         ORDER BY seq DESC LIMIT ?2 OFFSET ?3"
    ))?;
    let rows = stmt
        .query_map(params![agent_id, limit as i64, offset as i64], map_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok((rows, total as usize))
}

fn insert(
    conn: &Connection,
    agent_id: &str,
    seq: i64,
    content: &str,
    origin: &RevisionOrigin,
) -> Result<()> {
    let (actor_kind, actor_id, actor_name) = origin.actor.to_columns();
    let (trigger, restored_from) = origin.trigger.to_columns();
    conn.execute(
        "INSERT INTO core_revision
            (agent_id, seq, content, actor_kind, actor_id, actor_name, trigger, restored_from, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            agent_id,
            seq,
            content,
            actor_kind,
            actor_id,
            actor_name,
            trigger,
            restored_from,
            Utc::now().timestamp_millis(),
        ],
    )?;
    Ok(())
}

/// Lazily seeds seq 1 (`system`/`baseline`) from the document's current content when it has
/// no history yet (FR-018), so the first real change after upgrade has something to diff
/// against. A no-op when history already exists or there is no current content.
pub(crate) fn ensure_baseline(
    conn: &Connection,
    agent_id: &str,
    current: Option<&str>,
) -> Result<()> {
    let Some(current) = current else {
        return Ok(());
    };
    if latest(conn, agent_id)?.is_some() {
        return Ok(());
    }
    insert(
        conn,
        agent_id,
        1,
        current,
        &RevisionOrigin::system(RevisionTrigger::Baseline),
    )
}

/// Appends the next revision. Returns `Ok(None)` when `content` is identical to the latest
/// recorded revision (FR-003 no-op skip).
pub(crate) fn record(
    conn: &Connection,
    agent_id: &str,
    content: &str,
    origin: &RevisionOrigin,
    current_before_save: Option<&str>,
) -> Result<Option<i64>> {
    if agent_id.is_empty() {
        return Err(anyhow!("cannot record a CORE revision for an empty agent id"));
    }
    ensure_baseline(conn, agent_id, current_before_save)?;
    let next_seq = match latest(conn, agent_id)? {
        Some(latest) if latest.content == content => return Ok(None),
        Some(latest) => latest.seq + 1,
        None => 1,
    };
    insert(conn, agent_id, next_seq, content, origin)?;
    Ok(Some(next_seq))
}

pub(crate) fn delete_agent(conn: &Connection, agent_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM core_revision WHERE agent_id = ?1",
        params![agent_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::sqlite::init_revision_schema;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_revision_schema(&conn).unwrap();
        conn
    }

    fn agent_origin() -> RevisionOrigin {
        RevisionOrigin {
            actor: RevisionActor::Agent,
            trigger: RevisionTrigger::Conversation,
        }
    }

    #[test]
    fn first_record_with_prior_content_seeds_a_baseline() {
        let c = conn();
        let seq = record(&c, "a", "new", &agent_origin(), Some("old")).unwrap();
        assert_eq!(seq, Some(2));

        let (rows, total) = list(&c, "a", 0, 50).unwrap();
        assert_eq!(total, 2);
        assert_eq!(rows[0].seq, 2);
        assert_eq!(rows[0].content, "new");
        assert_eq!(rows[0].actor, RevisionActor::Agent);
        assert_eq!(rows[0].trigger, RevisionTrigger::Conversation);
        assert_eq!(rows[1].seq, 1);
        assert_eq!(rows[1].content, "old");
        assert_eq!(rows[1].actor, RevisionActor::System);
        assert_eq!(rows[1].trigger, RevisionTrigger::Baseline);
    }

    #[test]
    fn first_record_without_prior_content_starts_at_seq_1() {
        let c = conn();
        assert_eq!(record(&c, "a", "v1", &agent_origin(), None).unwrap(), Some(1));
        assert_eq!(record(&c, "a", "v2", &agent_origin(), Some("v1")).unwrap(), Some(2));
        assert_eq!(latest(&c, "a").unwrap().unwrap().seq, 2);
    }

    #[test]
    fn identical_content_is_a_no_op() {
        let c = conn();
        assert_eq!(record(&c, "a", "same", &agent_origin(), None).unwrap(), Some(1));
        assert_eq!(record(&c, "a", "same", &agent_origin(), Some("same")).unwrap(), None);
        assert_eq!(list(&c, "a", 0, 50).unwrap().1, 1);
    }

    #[test]
    fn list_is_newest_first_and_paginated() {
        let c = conn();
        for i in 1..=5 {
            record(&c, "a", &format!("v{i}"), &agent_origin(), None).unwrap();
        }
        let (rows, total) = list(&c, "a", 0, 2).unwrap();
        assert_eq!(total, 5);
        assert_eq!(rows.iter().map(|r| r.seq).collect::<Vec<_>>(), vec![5, 4]);
        let (rows, _) = list(&c, "a", 4, 2).unwrap();
        assert_eq!(rows.iter().map(|r| r.seq).collect::<Vec<_>>(), vec![1]);
        // limit is clamped into 1..=MAX_LIMIT
        assert_eq!(list(&c, "a", 0, 0).unwrap().0.len(), 1);
    }

    #[test]
    fn rollback_provenance_round_trips() {
        let c = conn();
        record(&c, "a", "v1", &agent_origin(), None).unwrap();
        let origin = RevisionOrigin {
            actor: RevisionActor::User {
                user_id: "u1".into(),
                username: "alice".into(),
            },
            trigger: RevisionTrigger::Rollback { restored_from: 1 },
        };
        record(&c, "a", "v2", &origin, Some("v1")).unwrap();
        let row = get(&c, "a", 2).unwrap().unwrap();
        assert_eq!(row.actor, origin.actor);
        assert_eq!(row.trigger, RevisionTrigger::Rollback { restored_from: 1 });
        assert_eq!(row.restored_from, Some(1));
        assert!(get(&c, "a", 99).unwrap().is_none());
    }

    #[test]
    fn delete_agent_removes_only_that_agent() {
        let c = conn();
        record(&c, "a", "v1", &agent_origin(), None).unwrap();
        record(&c, "b", "v1", &agent_origin(), None).unwrap();
        delete_agent(&c, "a").unwrap();
        assert_eq!(list(&c, "a", 0, 50).unwrap().1, 0);
        assert_eq!(list(&c, "b", 0, 50).unwrap().1, 1);
    }
}
