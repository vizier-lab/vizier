//! Memory concept-document version history — the `memory_revision` table.
//!
//! Synchronous helpers over a `&rusqlite::Connection`, called from `BundleMemoryStore`
//! (`write_memory` / `delete_memory` / `delete_bundle` / `import_bundle` /
//! `write_migrated_memory`) and the `MemoryStorage` history methods. CORE has its own,
//! separate module (`core_revision`); the two share only the value types in
//! `schema::revision`.
//!
//! A memory snapshot is stored as one *canonical* markdown text (research Decision 3): YAML
//! frontmatter holding only the user-authored fields (`title`, `tags`, `attachments`) plus the
//! body. Bookkeeping fields (`created_at`, `updated_at`, `read_count`, `keywords`, `relations`)
//! are deliberately excluded so an unchanged save compares equal and a diff only shows what
//! the author changed.

use anyhow::{Result, anyhow};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use crate::{
    schema::{RevisionActor, RevisionOrigin, RevisionTrigger, VizierAttachment},
    storage::memory_bundle::{parse_markdown_bytes, serialize_markdown},
};

pub const DEFAULT_LIMIT: usize = 50;
pub const MAX_LIMIT: usize = 200;

#[derive(Debug, Clone)]
pub struct MemoryRevisionRow {
    pub seq: i64,
    /// Canonical snapshot text; `None` only when `deleted` is `true`.
    pub content: Option<String>,
    pub deleted: bool,
    pub actor: RevisionActor,
    pub trigger: RevisionTrigger,
    pub restored_from: Option<i64>,
    pub created_at: i64,
}

/// The user-authored subset of `MemoryFrontMatter` that a snapshot preserves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionFrontMatter {
    pub title: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub attachments: Vec<VizierAttachment>,
}

pub(crate) fn memory_canonical(
    title: &str,
    tags: &[String],
    attachments: &[VizierAttachment],
    body: &str,
) -> Result<String> {
    let fm = RevisionFrontMatter {
        title: title.to_string(),
        tags: tags.to_vec(),
        attachments: attachments.to_vec(),
    };
    let bytes = serialize_markdown(&fm, body)?;
    Ok(String::from_utf8(bytes)?)
}

pub(crate) fn parse_memory_canonical(text: &str) -> Result<(RevisionFrontMatter, String)> {
    parse_markdown_bytes::<RevisionFrontMatter>(text.as_bytes())
}

fn clamp_limit(limit: usize) -> usize {
    limit.clamp(1, MAX_LIMIT)
}

const SELECT_COLUMNS: &str =
    "seq, content, deleted, actor_kind, actor_id, actor_name, trigger, restored_from, created_at";

fn map_row(row: &Row) -> rusqlite::Result<MemoryRevisionRow> {
    let deleted: i64 = row.get(2)?;
    let actor_kind: String = row.get(3)?;
    let trigger: String = row.get(6)?;
    let restored_from: Option<i64> = row.get(7)?;
    Ok(MemoryRevisionRow {
        seq: row.get(0)?,
        content: row.get(1)?,
        deleted: deleted != 0,
        actor: RevisionActor::from_columns(&actor_kind, row.get(4)?, row.get(5)?),
        trigger: RevisionTrigger::from_columns(&trigger, restored_from),
        restored_from,
        created_at: row.get(8)?,
    })
}

pub(crate) fn latest(
    conn: &Connection,
    agent_id: &str,
    bundle: &str,
    path: &str,
) -> Result<Option<MemoryRevisionRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM memory_revision
         WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3
         ORDER BY seq DESC LIMIT 1"
    ))?;
    Ok(stmt
        .query_row(params![agent_id, bundle, path], map_row)
        .optional()?)
}

pub(crate) fn get(
    conn: &Connection,
    agent_id: &str,
    bundle: &str,
    path: &str,
    seq: i64,
) -> Result<Option<MemoryRevisionRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM memory_revision
         WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3 AND seq = ?4"
    ))?;
    Ok(stmt
        .query_row(params![agent_id, bundle, path, seq], map_row)
        .optional()?)
}

/// Newest-first page plus the total row count for the document.
pub(crate) fn list(
    conn: &Connection,
    agent_id: &str,
    bundle: &str,
    path: &str,
    offset: usize,
    limit: usize,
) -> Result<(Vec<MemoryRevisionRow>, usize)> {
    let limit = clamp_limit(limit);
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM memory_revision WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3",
        params![agent_id, bundle, path],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM memory_revision
         WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3
         ORDER BY seq DESC LIMIT ?4 OFFSET ?5"
    ))?;
    let rows = stmt
        .query_map(
            params![agent_id, bundle, path, limit as i64, offset as i64],
            map_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok((rows, total as usize))
}

#[allow(clippy::too_many_arguments)]
fn insert(
    conn: &Connection,
    agent_id: &str,
    bundle: &str,
    path: &str,
    seq: i64,
    content: Option<&str>,
    origin: &RevisionOrigin,
) -> Result<()> {
    let (actor_kind, actor_id, actor_name) = origin.actor.to_columns();
    let (trigger, restored_from) = origin.trigger.to_columns();
    conn.execute(
        "INSERT INTO memory_revision
            (agent_id, bundle, path, seq, content, deleted,
             actor_kind, actor_id, actor_name, trigger, restored_from, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            agent_id,
            bundle,
            path,
            seq,
            content,
            content.is_none() as i64,
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

/// Lazily seeds seq 1 (`system`/`baseline`) from the document's current canonical text when it
/// has no history yet (FR-018). A no-op when history already exists or there is no current
/// document.
pub(crate) fn ensure_baseline(
    conn: &Connection,
    agent_id: &str,
    bundle: &str,
    path: &str,
    current: Option<&str>,
) -> Result<()> {
    let Some(current) = current else {
        return Ok(());
    };
    if latest(conn, agent_id, bundle, path)?.is_some() {
        return Ok(());
    }
    insert(
        conn,
        agent_id,
        bundle,
        path,
        1,
        Some(current),
        &RevisionOrigin::system(RevisionTrigger::Baseline),
    )
}

/// Appends the next revision. `content = None` records a deletion entry. Returns `Ok(None)`
/// when `(content, deleted)` equals the latest recorded revision (FR-003 no-op skip; also
/// prevents two consecutive deletion entries).
pub(crate) fn record(
    conn: &Connection,
    agent_id: &str,
    bundle: &str,
    path: &str,
    content: Option<&str>,
    origin: &RevisionOrigin,
    current_before_save: Option<&str>,
) -> Result<Option<i64>> {
    if path.is_empty() {
        return Err(anyhow!("cannot record a memory revision for an empty path"));
    }
    ensure_baseline(conn, agent_id, bundle, path, current_before_save)?;
    let next_seq = match latest(conn, agent_id, bundle, path)? {
        Some(latest) if latest.deleted == content.is_none() && latest.content.as_deref() == content => {
            return Ok(None);
        }
        Some(latest) => latest.seq + 1,
        // Nothing to record: the document never existed and this is a delete.
        None if content.is_none() => return Ok(None),
        None => 1,
    };
    insert(conn, agent_id, bundle, path, next_seq, content, origin)?;
    Ok(Some(next_seq))
}

pub(crate) fn delete_agent(conn: &Connection, agent_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM memory_revision WHERE agent_id = ?1",
        params![agent_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{schema::VizierAttachmentContent, storage::sqlite::init_revision_schema};

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

    const A: &str = "agent";
    const B: &str = "default";
    const P: &str = "friends/bred";

    #[test]
    fn first_record_with_prior_content_seeds_a_baseline() {
        let c = conn();
        let seq = record(&c, A, B, P, Some("new"), &agent_origin(), Some("old")).unwrap();
        assert_eq!(seq, Some(2));
        let (rows, total) = list(&c, A, B, P, 0, 50).unwrap();
        assert_eq!(total, 2);
        assert_eq!(rows[0].seq, 2);
        assert_eq!(rows[0].content.as_deref(), Some("new"));
        assert!(!rows[0].deleted);
        assert_eq!(rows[1].seq, 1);
        assert_eq!(rows[1].content.as_deref(), Some("old"));
        assert_eq!(rows[1].actor, RevisionActor::System);
        assert_eq!(rows[1].trigger, RevisionTrigger::Baseline);
    }

    #[test]
    fn identical_content_is_a_no_op() {
        let c = conn();
        assert_eq!(record(&c, A, B, P, Some("x"), &agent_origin(), None).unwrap(), Some(1));
        assert_eq!(record(&c, A, B, P, Some("x"), &agent_origin(), Some("x")).unwrap(), None);
        assert_eq!(list(&c, A, B, P, 0, 50).unwrap().1, 1);
    }

    #[test]
    fn empty_path_is_rejected() {
        let c = conn();
        assert!(record(&c, A, B, "", Some("x"), &agent_origin(), None).is_err());
    }

    #[test]
    fn deletion_then_content_then_deletion_sequence() {
        let c = conn();
        assert_eq!(record(&c, A, B, P, Some("v1"), &agent_origin(), None).unwrap(), Some(1));
        // deletion entry
        assert_eq!(record(&c, A, B, P, None, &agent_origin(), Some("v1")).unwrap(), Some(2));
        let latest_row = latest(&c, A, B, P).unwrap().unwrap();
        assert!(latest_row.deleted);
        assert!(latest_row.content.is_none());
        // a second consecutive deletion is a no-op
        assert_eq!(record(&c, A, B, P, None, &agent_origin(), None).unwrap(), None);
        // recreating with the same content as v1 is still a new revision (latest is a delete)
        assert_eq!(record(&c, A, B, P, Some("v1"), &agent_origin(), None).unwrap(), Some(3));
        let (rows, total) = list(&c, A, B, P, 0, 50).unwrap();
        assert_eq!(total, 3);
        assert_eq!(rows.iter().map(|r| r.seq).collect::<Vec<_>>(), vec![3, 2, 1]);
    }

    #[test]
    fn deleting_a_never_recorded_document_records_nothing() {
        let c = conn();
        assert_eq!(record(&c, A, B, P, None, &agent_origin(), None).unwrap(), None);
        assert_eq!(list(&c, A, B, P, 0, 50).unwrap().1, 0);
    }

    #[test]
    fn list_is_newest_first_with_total() {
        let c = conn();
        for i in 1..=4 {
            record(&c, A, B, P, Some(&format!("v{i}")), &agent_origin(), None).unwrap();
        }
        // a different path in the same bundle is independent
        record(&c, A, B, "other", Some("o"), &agent_origin(), None).unwrap();
        let (rows, total) = list(&c, A, B, P, 1, 2).unwrap();
        assert_eq!(total, 4);
        assert_eq!(rows.iter().map(|r| r.seq).collect::<Vec<_>>(), vec![3, 2]);
        assert!(get(&c, A, B, P, 4).unwrap().is_some());
        assert!(get(&c, A, B, P, 5).unwrap().is_none());
    }

    #[test]
    fn delete_agent_empties_the_table_for_that_agent() {
        let c = conn();
        record(&c, A, B, P, Some("v1"), &agent_origin(), None).unwrap();
        record(&c, "other-agent", B, P, Some("v1"), &agent_origin(), None).unwrap();
        delete_agent(&c, A).unwrap();
        assert_eq!(list(&c, A, B, P, 0, 50).unwrap().1, 0);
        assert_eq!(list(&c, "other-agent", B, P, 0, 50).unwrap().1, 1);
    }

    #[test]
    fn canonical_round_trips_title_tags_attachments_and_body() {
        let attachments = vec![VizierAttachment {
            filename: "photo.png".into(),
            content: VizierAttachmentContent::Local("files/photo.png".into()),
        }];
        let tags = vec!["friend".to_string(), "runner".to_string()];
        let body = "Bred likes running.\n\nSee [[friends/alice]].\n";
        let text = memory_canonical("Bred", &tags, &attachments, body).unwrap();
        assert!(text.starts_with("---\ntitle: Bred\n"));
        assert!(text.contains("tags:\n- friend\n- runner\n"));

        let (fm, parsed_body) = parse_memory_canonical(&text).unwrap();
        assert_eq!(fm.title, "Bred");
        assert_eq!(fm.tags, tags);
        assert_eq!(fm.attachments.len(), 1);
        assert_eq!(fm.attachments[0].filename, "photo.png");
        assert_eq!(parsed_body, body);
    }

    #[test]
    fn canonical_is_stable_for_equal_inputs() {
        let a = memory_canonical("T", &["x".into()], &[], "body").unwrap();
        let b = memory_canonical("T", &["x".into()], &[], "body").unwrap();
        assert_eq!(a, b);
        let c = memory_canonical("T", &["y".into()], &[], "body").unwrap();
        assert_ne!(a, c);
    }
}
