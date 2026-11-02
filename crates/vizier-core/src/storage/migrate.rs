//! `vizier storage migrate` (spec 045, contracts/configuration-and-cli.md): copy a stopped
//! SQLite + local-files workspace into the configured PostgreSQL database and file store.
//!
//! The source is opened read-only and never changed. The destination schema is created by the
//! normal migrations; tables are then copied in foreign-key order, each batch committed together
//! with its cursor in `storage_migration`, so an interrupted run resumes without duplicates.
//! Files are copied with their SHA-256 checked on the copy. The report holds counts, sizes and
//! checksums only. Tracked for removal with the other migration code in #82.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::VizierConfig;
use crate::config::storage::{ObjectsConfig, StorageConfig};
use crate::storage::blob::{BlobKey, BlobStore, LocalBlobStore};
use crate::storage::sql::{Backend, Connection, SqlStore, SqlValue, params, params_from_iter};
use crate::storage::sqlite::SqlStorage;

/// Rows per committed batch.
const BATCH: i64 = 500;
/// Tables the destination keeps for itself.
const OWN_TABLES: &[&str] = &["vizier_schema_migrations", "storage_migration"];

#[derive(Debug, Clone)]
pub struct MigrateOptions {
    pub source_workspace: PathBuf,
    /// Empty the destination (tables only) before copying, even after a completed run.
    pub reset_destination: bool,
}

/// How a run ended when it did not fail.
#[derive(Debug)]
pub enum MigrateOutcome {
    Done(MigrationReport),
    /// Nothing was changed; the message says why (exit code 2).
    Refused(String),
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MigrationReport {
    pub migration_id: String,
    pub tables: BTreeMap<String, TableReport>,
    pub vectors: BTreeMap<String, TableReport>,
    pub blobs: BlobReport,
    pub duration_ms: u128,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TableReport {
    pub rows: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BlobReport {
    pub count: u64,
    pub bytes: u64,
    pub verified: u64,
    /// Files were already where the destination reads them (same workspace, local files).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub skipped_same_location: bool,
}

/// Where a run stopped: the phase and the last copied key of it.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Cursor {
    phase: String,
    table: Option<String>,
    rowid: Option<i64>,
    blob: Option<String>,
}

pub async fn migrate(dest: &VizierConfig, options: &MigrateOptions) -> Result<MigrateOutcome> {
    let started = std::time::Instant::now();
    if !matches!(dest.storage, StorageConfig::Postgres(_)) {
        return Ok(MigrateOutcome::Refused(
            "the destination must be PostgreSQL (--storage postgres)".into(),
        ));
    }
    let source_dir = options.source_workspace.clone();

    // A running vizier holds this lock; the copy must see a quiet database.
    let _lock = match crate::runtime::WorkspaceLock::acquire(&source_dir) {
        Ok(lock) => lock,
        Err(e) => return Ok(MigrateOutcome::Refused(format!("{}; stop it first", e.0))),
    };
    let Some(source) = SqlStorage::open_read_only(&source_dir)? else {
        return Ok(MigrateOutcome::Refused(format!(
            "{} has no vizier database",
            source_dir.display()
        )));
    };
    if let Some(problem) = schema_mismatch(&source)? {
        return Ok(MigrateOutcome::Refused(format!(
            "{problem}; start this workspace once with this version of vizier, stop it, then migrate"
        )));
    }
    let fingerprint = fingerprint(&source_dir, &source)?;

    let store = crate::storage::open_store(dest)?;
    if store.backend() != Backend::Postgres {
        return Ok(MigrateOutcome::Refused(
            "the destination must be PostgreSQL".into(),
        ));
    }
    let conn = store.lock()?;
    ensure_migration_table(&conn)?;

    let (id, mut cursor, mut report) = match start(&conn, &fingerprint, options.reset_destination)?
    {
        Start::Refused(why) => return Ok(MigrateOutcome::Refused(why)),
        Start::Run { id, cursor, report } => (id, cursor, report),
    };
    report.migration_id = id.clone();

    if phase_rank(&cursor.phase) <= phase_rank("tables") {
        copy_tables(&source, &conn, &id, &mut cursor, &mut report)?;
        resync_identities(&conn)?;
        cursor = Cursor {
            phase: "vectors".into(),
            ..Default::default()
        };
        save(&conn, &id, &cursor, &report)?;
    }
    if phase_rank(&cursor.phase) <= phase_rank("vectors") {
        copy_vectors(&source, &conn, &mut report)?;
        cursor = Cursor {
            phase: "blobs".into(),
            ..Default::default()
        };
        save(&conn, &id, &cursor, &report)?;
    }
    drop(conn);
    if phase_rank(&cursor.phase) <= phase_rank("blobs") {
        copy_blobs(dest, &source_dir, &store, &id, &mut cursor, &mut report).await?;
    }

    report.duration_ms = started.elapsed().as_millis();
    let conn = store.lock()?;
    cursor = Cursor {
        phase: "done".into(),
        ..Default::default()
    };
    save(&conn, &id, &cursor, &report)?;
    conn.execute(
        "UPDATE storage_migration SET finished_at = ?2 WHERE id = ?1",
        params![id, now_ms()],
    )?;
    Ok(MigrateOutcome::Done(report))
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn phase_rank(phase: &str) -> u8 {
    match phase {
        "tables" => 1,
        "vectors" => 2,
        "blobs" => 3,
        "done" => 4,
        _ => 0,
    }
}

// ---- source checks -------------------------------------------------------------------------

/// The first table or column this binary's schema has that the source lacks.
fn schema_mismatch(source: &rusqlite::Connection) -> Result<Option<String>> {
    let expected = SqlStorage::expected_schema()?;
    for table in sqlite_tables(&expected)? {
        let want = sqlite_columns(&expected, &table)?;
        let have = sqlite_columns(source, &table)?;
        if have.is_empty() {
            return Ok(Some(format!("the source has no table {table}")));
        }
        if let Some(missing) = want.iter().find(|c| !have.contains(*c)) {
            return Ok(Some(format!(
                "the source table {table} has no column {missing}"
            )));
        }
    }
    Ok(None)
}

/// Ordinary tables (no virtual tables, their shadow tables or SQLite's own).
fn sqlite_tables(conn: &rusqlite::Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
           AND sql NOT LIKE 'CREATE VIRTUAL TABLE%'
         ORDER BY name",
    )?;
    let names: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    // Shadow tables of FTS5/vec0 virtual tables are named `<virtual>_<suffix>`.
    let virtuals: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND sql LIKE 'CREATE VIRTUAL TABLE%'")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(names
        .into_iter()
        .filter(|n| !virtuals.iter().any(|v| n.starts_with(&format!("{v}_"))))
        .collect())
}

fn sqlite_columns(conn: &rusqlite::Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    Ok(stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<_>>()?)
}

/// Identifies the source: its path, and every table's row count and highest rowid.
fn fingerprint(dir: &Path, source: &rusqlite::Connection) -> Result<String> {
    let mut hash = Sha256::new();
    hash.update(
        dir.canonicalize()
            .unwrap_or_else(|_| dir.to_path_buf())
            .to_string_lossy()
            .as_bytes(),
    );
    for table in sqlite_tables(source)? {
        let (count, max): (i64, Option<i64>) = source.query_row(
            &format!("SELECT COUNT(*), MAX(rowid) FROM \"{table}\""),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        hash.update(format!("{table}:{count}:{}\n", max.unwrap_or(0)).as_bytes());
    }
    Ok(hex(&hash.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---- run bookkeeping -----------------------------------------------------------------------

fn ensure_migration_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS storage_migration (
            id TEXT PRIMARY KEY,
            source_fingerprint TEXT NOT NULL UNIQUE,
            phase TEXT NOT NULL,
            cursor TEXT NOT NULL,
            report TEXT NOT NULL,
            started_at BIGINT NOT NULL,
            updated_at BIGINT NOT NULL,
            finished_at BIGINT
        )",
    )?;
    Ok(())
}

enum Start {
    Refused(String),
    Run {
        id: String,
        cursor: Cursor,
        report: MigrationReport,
    },
}

fn start(conn: &Connection, fingerprint: &str, reset: bool) -> Result<Start> {
    let existing: Option<(String, String, String, String)> = conn
        .query_row(
            "SELECT id, phase, cursor, report FROM storage_migration WHERE source_fingerprint = ?1",
            params![fingerprint],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map(Some)
        .or_else(|e| match e {
            crate::storage::sql::Error::QueryReturnedNoRows => Ok(None),
            e => Err(e),
        })?;

    match existing {
        Some((_, phase, _, _)) if phase == "done" && !reset => {
            return Ok(Start::Refused(
                "this workspace was already migrated into this database (pass --reset-destination to copy it again)".into(),
            ));
        }
        Some((id, _, cursor, report)) if !reset => {
            tracing::info!("resuming migration {id}");
            return Ok(Start::Run {
                id,
                cursor: serde_json::from_str(&cursor)?,
                report: serde_json::from_str(&report)?,
            });
        }
        _ => {}
    }

    let tables = destination_tables(conn)?;
    if reset {
        if !tables.is_empty() {
            let list = tables
                .iter()
                .map(|t| quote(t))
                .collect::<Vec<_>>()
                .join(", ");
            conn.execute_batch(&format!("TRUNCATE {list} RESTART IDENTITY CASCADE"))?;
        }
        conn.execute("DELETE FROM storage_migration", ())?;
    } else {
        for table in &tables {
            let rows: i64 =
                conn.query_row(&format!("SELECT COUNT(*) FROM {}", quote(table)), (), |r| {
                    r.get(0)
                })?;
            if rows > 0 {
                return Ok(Start::Refused(format!(
                    "the destination database is not empty (table {table}); use an empty database or pass --reset-destination"
                )));
            }
        }
    }

    let id = uuid::Uuid::new_v4().to_string();
    let cursor = Cursor {
        phase: "tables".into(),
        ..Default::default()
    };
    let report = MigrationReport::default();
    conn.execute(
        "INSERT INTO storage_migration (id, source_fingerprint, phase, cursor, report, started_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![id, fingerprint, cursor.phase, serde_json::to_string(&cursor)?, serde_json::to_string(&report)?, now_ms()],
    )?;
    Ok(Start::Run { id, cursor, report })
}

fn save(conn: &Connection, id: &str, cursor: &Cursor, report: &MigrationReport) -> Result<()> {
    conn.execute(
        "UPDATE storage_migration SET phase = ?2, cursor = ?3, report = ?4, updated_at = ?5 WHERE id = ?1",
        params![id, cursor.phase, serde_json::to_string(cursor)?, serde_json::to_string(report)?, now_ms()],
    )?;
    Ok(())
}

fn quote(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

// ---- tables --------------------------------------------------------------------------------

/// The destination's application tables in foreign-key order (parents first).
fn destination_tables(conn: &Connection) -> Result<Vec<String>> {
    let names: Vec<String> = conn
        .query_all(
            "SELECT table_name FROM information_schema.tables
             WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'
             ORDER BY table_name",
            (),
        )?
        .iter()
        .map(|r| r.get(0))
        .collect::<crate::storage::sql::Result<_>>()?;
    let names: Vec<String> = names
        .into_iter()
        .filter(|n| !OWN_TABLES.contains(&n.as_str()) && !n.starts_with("document_index"))
        .collect();

    let edges: Vec<(String, String)> = conn
        .query_all(
            "SELECT child.relname, parent.relname
             FROM pg_constraint c
             JOIN pg_class child ON child.oid = c.conrelid
             JOIN pg_class parent ON parent.oid = c.confrelid
             JOIN pg_namespace n ON n.oid = child.relnamespace
             WHERE c.contype = 'f' AND n.nspname = current_schema()",
            (),
        )?
        .iter()
        .map(|r| Ok((r.get(0)?, r.get(1)?)))
        .collect::<crate::storage::sql::Result<_>>()?;

    // Kahn's algorithm; a self-reference or cycle falls back to name order.
    let mut parents: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (child, parent) in &edges {
        if child != parent {
            parents
                .entry(child.as_str())
                .or_default()
                .insert(parent.as_str());
        }
    }
    let mut done: HashSet<String> = HashSet::new();
    let mut order = Vec::with_capacity(names.len());
    while order.len() < names.len() {
        let ready: Vec<&String> = names
            .iter()
            .filter(|n| !done.contains(*n))
            .filter(|n| {
                parents.get(n.as_str()).is_none_or(|ps| {
                    ps.iter()
                        .all(|p| done.contains(*p) || !names.iter().any(|x| x == p))
                })
            })
            .collect();
        let next: Vec<String> = if ready.is_empty() {
            names
                .iter()
                .filter(|n| !done.contains(*n))
                .take(1)
                .cloned()
                .collect()
        } else {
            ready.into_iter().cloned().collect()
        };
        for n in next {
            done.insert(n.clone());
            order.push(n);
        }
    }
    Ok(order)
}

/// Columns the destination accepts values for (generated columns are computed by PostgreSQL).
fn destination_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    Ok(conn
        .query_all(
            "SELECT column_name FROM information_schema.columns
             WHERE table_schema = current_schema() AND table_name = ?1 AND is_generated = 'NEVER'
             ORDER BY ordinal_position",
            params![table],
        )?
        .iter()
        .map(|r| r.get(0))
        .collect::<crate::storage::sql::Result<_>>()?)
}

fn copy_tables(
    source: &rusqlite::Connection,
    conn: &Connection,
    id: &str,
    cursor: &mut Cursor,
    report: &mut MigrationReport,
) -> Result<()> {
    let source_tables: HashSet<String> = sqlite_tables(source)?.into_iter().collect();
    let tables = destination_tables(conn)?;
    let resume_at = cursor.table.clone();
    let mut skipping = resume_at.is_some();

    for table in tables {
        if skipping {
            if Some(&table) != resume_at.as_ref() {
                continue;
            }
            skipping = false;
        } else {
            cursor.table = Some(table.clone());
            cursor.rowid = None;
        }
        if !source_tables.contains(&table) {
            continue;
        }
        let have = sqlite_columns(source, &table)?;
        let columns: Vec<String> = destination_columns(conn, &table)?
            .into_iter()
            .filter(|c| have.contains(c))
            .collect();
        if columns.is_empty() {
            continue;
        }
        let select = format!(
            "SELECT rowid, {} FROM {} WHERE rowid > ?1 ORDER BY rowid LIMIT {BATCH}",
            columns
                .iter()
                .map(|c| quote(c))
                .collect::<Vec<_>>()
                .join(", "),
            quote(&table)
        );
        let insert = format!(
            "INSERT INTO {} ({}) OVERRIDING SYSTEM VALUE VALUES ({})",
            quote(&table),
            columns
                .iter()
                .map(|c| quote(c))
                .collect::<Vec<_>>()
                .join(", "),
            (1..=columns.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(", ")
        );

        loop {
            let after = cursor.rowid.unwrap_or(0);
            let mut stmt = source.prepare_cached(&select)?;
            let width = columns.len();
            let batch: Vec<(i64, Vec<SqlValue>)> = stmt
                .query_map([after], |row| {
                    let rowid: i64 = row.get(0)?;
                    let mut values = Vec::with_capacity(width);
                    for i in 0..width {
                        values.push(match row.get_ref(i + 1)? {
                            rusqlite::types::ValueRef::Null => SqlValue::Null,
                            rusqlite::types::ValueRef::Integer(v) => SqlValue::Integer(v),
                            rusqlite::types::ValueRef::Real(v) => SqlValue::Real(v),
                            rusqlite::types::ValueRef::Text(v) => {
                                SqlValue::Text(String::from_utf8_lossy(v).into_owned())
                            }
                            rusqlite::types::ValueRef::Blob(v) => SqlValue::Blob(v.to_vec()),
                        });
                    }
                    Ok((rowid, values))
                })?
                .collect::<rusqlite::Result<_>>()?;
            if batch.is_empty() {
                break;
            }
            let tx = conn.transaction()?;
            for (_, values) in &batch {
                tx.execute(&insert, params_from_iter(values.iter()))
                    .map_err(|e| anyhow!("copying {table}: {e}"))?;
            }
            let last = batch.last().map(|(r, _)| *r);
            cursor.rowid = last;
            report.tables.entry(table.clone()).or_default().rows += batch.len() as u64;
            save(&tx, id, cursor, report)?;
            tx.commit()?;
            tracing::info!(table, rows = report.tables[&table].rows, "copied");
        }
    }
    Ok(())
}

/// Point every identity sequence past the copied ids.
fn resync_identities(conn: &Connection) -> Result<()> {
    let identities = conn.query_all(
        "SELECT table_name, column_name FROM information_schema.columns
         WHERE table_schema = current_schema() AND is_identity = 'YES'",
        (),
    )?;
    for row in identities {
        let (table, column): (String, String) = (row.get(0)?, row.get(1)?);
        conn.query_row(
            &format!(
                "SELECT setval(pg_get_serial_sequence(?1, ?2), COALESCE(MAX({}), 0) + 1, false) FROM {}",
                quote(&column),
                quote(&table)
            ),
            params![quote(&table), column],
            |_| Ok(()),
        )?;
    }
    Ok(())
}

// ---- vectors -------------------------------------------------------------------------------

/// Copy every sqlite-vec table into the pgvector table of the same width. Vectors are copied
/// whole (a re-run of this phase replaces them), so no cursor is kept.
fn copy_vectors(
    source: &rusqlite::Connection,
    conn: &Connection,
    report: &mut MigrationReport,
) -> Result<()> {
    let tables: Vec<(String, String)> = source
        .prepare(
            "SELECT name, sql FROM sqlite_master
             WHERE type = 'table' AND sql LIKE 'CREATE VIRTUAL TABLE%vec0%'",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (name, ddl) in tables {
        let dimension = ddl
            .split("float[")
            .nth(1)
            .and_then(|rest| rest.split(']').next())
            .and_then(|n| n.trim().parse::<usize>().ok())
            .ok_or_else(|| anyhow!("cannot read the width of {name}"))?;
        let target = crate::indexer::postgres::ensure_table(conn, dimension)?;
        conn.execute(&format!("DELETE FROM {target}"), ())?;
        let insert = crate::indexer::postgres::insert_sql(&target);
        let mut after = 0i64;
        let mut copied = 0u64;
        loop {
            let batch: Vec<(i64, String, String, Vec<u8>)> = source
                .prepare_cached(&format!(
                    "SELECT rowid, context, path, embedding FROM \"{name}\" WHERE rowid > ?1 ORDER BY rowid LIMIT {BATCH}"
                ))?
                .query_map([after], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<rusqlite::Result<_>>()?;
            let Some(last) = batch.last().map(|b| b.0) else {
                break;
            };
            let tx = conn.transaction()?;
            for (_, context, path, bytes) in &batch {
                let floats: Vec<f64> = bytes
                    .chunks_exact(4)
                    .map(|c| f64::from(f32::from_le_bytes([c[0], c[1], c[2], c[3]])))
                    .collect();
                tx.execute(
                    &insert,
                    params![
                        crate::indexer::postgres::vector_literal(&floats),
                        context,
                        path
                    ],
                )?;
            }
            tx.commit()?;
            copied += batch.len() as u64;
            after = last;
        }
        report.vectors.insert(target, TableReport { rows: copied });
    }
    Ok(())
}

// ---- files ---------------------------------------------------------------------------------

/// The keys a workspace stores user data under: uploads, artifact versions, memory documents.
async fn source_keys(source: &LocalBlobStore) -> Result<Vec<(BlobKey, u64)>> {
    let mut keys = Vec::new();
    for prefix in ["uploads", "artifacts", "agents"] {
        for meta in source.list(&BlobKey::new(prefix)?).await? {
            let k = meta.key.as_str();
            if prefix == "agents" && !k.split('/').nth(2).is_some_and(|seg| seg == "memory") {
                continue;
            }
            keys.push((meta.key, meta.size));
        }
    }
    keys.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    Ok(keys)
}

async fn copy_blobs(
    dest: &VizierConfig,
    source_dir: &Path,
    store: &SqlStore,
    id: &str,
    cursor: &mut Cursor,
    report: &mut MigrationReport,
) -> Result<()> {
    if matches!(dest.objects, ObjectsConfig::Local) {
        let same = Path::new(&dest.workspace).canonicalize().ok() == source_dir.canonicalize().ok();
        if same {
            report.blobs.skipped_same_location = true;
            return Ok(());
        }
    }
    let target = crate::storage::blob::open(dest).await?;
    let source = LocalBlobStore::new(source_dir.to_path_buf());
    let resume_after = cursor.blob.clone();

    for (key, size) in source_keys(&source).await? {
        if resume_after
            .as_deref()
            .is_some_and(|done| key.as_str() <= done)
        {
            continue;
        }
        let digest = copy_one(&source, &*target, &key, size).await?;
        verify_one(&*target, &key, size, &digest).await?;
        report.blobs.count += 1;
        report.blobs.bytes += size;
        report.blobs.verified += 1;
        cursor.blob = Some(key.as_str().to_string());
        if report.blobs.count % 50 == 0 {
            save(&store.lock()?, id, cursor, report)?;
        }
    }
    save(&store.lock()?, id, cursor, report)?;
    Ok(())
}

/// Stream one file across, returning the SHA-256 of what was read.
async fn copy_one(
    source: &LocalBlobStore,
    target: &dyn BlobStore,
    key: &BlobKey,
    size: u64,
) -> Result<[u8; 32]> {
    let (_, body) = source
        .get_range(key, None)
        .await?
        .ok_or_else(|| anyhow!("{key} disappeared from the source"))?;
    let hasher = Arc::new(parking_lot::Mutex::new(Sha256::new()));
    let tee = hasher.clone();
    let body = Box::pin(body.map(move |chunk| {
        if let Ok(bytes) = &chunk {
            tee.lock().update(bytes);
        }
        chunk
    }));
    target.put_stream(key, body, Some(size)).await?;
    let digest = hasher.lock().clone().finalize();
    Ok(digest.into())
}

/// Read the copy back: same size, same SHA-256.
async fn verify_one(
    target: &dyn BlobStore,
    key: &BlobKey,
    size: u64,
    digest: &[u8; 32],
) -> Result<()> {
    let (meta, mut body) = target
        .get_range(key, None)
        .await?
        .ok_or_else(|| anyhow!("{key} is missing from the destination after copying"))?;
    if meta.size != size {
        return Err(anyhow!(
            "{key}: destination has {} bytes, source {size}",
            meta.size
        ));
    }
    let mut hash = Sha256::new();
    while let Some(chunk) = body.next().await {
        hash.update(&chunk?);
    }
    if hash.finalize().as_slice() != digest {
        return Err(anyhow!(
            "{key}: destination content differs from the source"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_and_shadow_tables_are_not_copied_as_tables() {
        let conn = SqlStorage::expected_schema().unwrap();
        let tables = sqlite_tables(&conn).unwrap();
        assert!(tables.iter().any(|t| t == "session_history"));
        assert!(
            !tables.iter().any(|t| t.starts_with("memory_fts")),
            "{tables:?}"
        );
    }

    #[test]
    fn a_current_source_has_no_schema_mismatch_and_an_old_one_does() {
        let current = SqlStorage::expected_schema().unwrap();
        assert_eq!(schema_mismatch(&current).unwrap(), None);

        let old = rusqlite::Connection::open_in_memory().unwrap();
        old.execute_batch("CREATE TABLE state (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .unwrap();
        assert!(schema_mismatch(&old).unwrap().is_some());
    }

    #[test]
    fn persisted_report_round_trips_when_optional_false_fields_are_omitted() {
        let json = serde_json::to_string(&MigrationReport::default()).unwrap();
        assert!(!json.contains("skipped_same_location"));
        let report: MigrationReport = serde_json::from_str(&json).unwrap();
        assert!(!report.blobs.skipped_same_location);
    }
}
