use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::Connection;

use crate::storage::VizierStorageProvider;
use crate::storage::document::DocumentStore;
use crate::utils::build_path;

mod agent;
pub(crate) mod core_revision;
mod dream_journal;
mod global_config;
mod history;
mod memory;
pub(crate) mod memory_revision;
mod provider;
mod session;
mod session_file;
mod state;
mod task;
mod user;

#[derive(Clone)]
pub struct SqliteStorage {
    pub conn: Arc<Mutex<Connection>>,
    pub document_store: Arc<dyn DocumentStore>,
}

/// The Memory Graph Index tables (`memory_node`/`memory_edge`, data-model.md), split out from
/// `init_schema` so unit tests (`src/storage/memory_bundle.rs`) can stand up just these tables
/// against an in-memory connection without the rest of the application schema.
pub fn init_memory_graph_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS memory_node (
            agent_id TEXT NOT NULL,
            bundle TEXT NOT NULL,
            path TEXT NOT NULL,
            slug TEXT NOT NULL,
            title TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            attachment_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            read_count INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (agent_id, bundle, path)
        );
        CREATE INDEX IF NOT EXISTS idx_memory_node_agent_bundle ON memory_node(agent_id, bundle);

        CREATE TABLE IF NOT EXISTS memory_edge (
            agent_id TEXT NOT NULL,
            source_bundle TEXT NOT NULL,
            source_path TEXT NOT NULL,
            target_bundle TEXT NOT NULL,
            target_path TEXT,
            target_kind TEXT NOT NULL,
            broken INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_memory_edge_source ON memory_edge(agent_id, source_bundle, source_path);
        CREATE INDEX IF NOT EXISTS idx_memory_edge_target ON memory_edge(agent_id, target_bundle, target_path);
        ",
    )?;
    Ok(())
}

/// The append-only version-history tables (`core_revision`/`memory_revision`,
/// specs/006-memory-version-history/data-model.md). Split out like `init_memory_graph_schema`
/// so the `core_revision`/`memory_revision`/`memory_bundle` unit tests can stand them up on an
/// in-memory connection.
pub fn init_revision_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS core_revision (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id TEXT NOT NULL,
            seq INTEGER NOT NULL,
            content TEXT NOT NULL,
            actor_kind TEXT NOT NULL,
            actor_id TEXT,
            actor_name TEXT,
            trigger TEXT NOT NULL,
            restored_from INTEGER,
            created_at INTEGER NOT NULL,
            UNIQUE(agent_id, seq)
        );
        CREATE INDEX IF NOT EXISTS idx_core_rev_doc ON core_revision(agent_id, seq DESC);

        CREATE TABLE IF NOT EXISTS memory_revision (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id TEXT NOT NULL,
            bundle TEXT NOT NULL,
            path TEXT NOT NULL,
            seq INTEGER NOT NULL,
            content TEXT,
            deleted INTEGER NOT NULL DEFAULT 0,
            actor_kind TEXT NOT NULL,
            actor_id TEXT,
            actor_name TEXT,
            trigger TEXT NOT NULL,
            restored_from INTEGER,
            created_at INTEGER NOT NULL,
            UNIQUE(agent_id, bundle, path, seq)
        );
        CREATE INDEX IF NOT EXISTS idx_mem_rev_doc ON memory_revision(agent_id, bundle, path, seq DESC);
        CREATE INDEX IF NOT EXISTS idx_mem_rev_agent ON memory_revision(agent_id);
        ",
    )?;
    Ok(())
}

impl SqliteStorage {
    pub fn open_connection(workspace: &str) -> Result<Connection> {
        let db_path = build_path(workspace, &[".runtime"]);
        std::fs::create_dir_all(&db_path)?;

        let db_file = db_path.join("vizier.db");

        // Register sqlite-vec extension before opening any connection
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
                *const (),
                unsafe extern "C" fn(
                    *mut rusqlite::ffi::sqlite3,
                    *mut *mut std::ffi::c_char,
                    *const rusqlite::ffi::sqlite3_api_routines,
                ) -> i32,
            >(sqlite_vec::sqlite3_vec_init as *const ())));
        }

        let conn = Connection::open(&db_file)?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        conn.execute_batch("PRAGMA busy_timeout=5000;")?;

        Self::init_schema(&conn)?;

        Ok(conn)
    }

    pub fn new(conn: Arc<Mutex<Connection>>, document_store: Arc<dyn DocumentStore>) -> Self {
        Self {
            conn,
            document_store,
        }
    }

    pub fn bundle_store(&self) -> crate::storage::memory_bundle::BundleMemoryStore {
        crate::storage::memory_bundle::BundleMemoryStore::new(
            self.document_store.clone(),
            self.conn.clone(),
        )
    }

    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS memory (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                slug TEXT NOT NULL,
                visibility TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memory_agent ON memory(agent_id);
            CREATE INDEX IF NOT EXISTS idx_memory_visibility ON memory(visibility);

            CREATE TABLE IF NOT EXISTS task (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                slug TEXT NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 1,
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_task_agent ON task(agent_id);

            CREATE TABLE IF NOT EXISTS session_history (
                uid TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                topic TEXT,
                timestamp INTEGER NOT NULL,
                content_type TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_sh_session ON session_history(agent_id, channel, topic);
            CREATE INDEX IF NOT EXISTS idx_sh_time ON session_history(timestamp);
            CREATE INDEX IF NOT EXISTS idx_sh_agent_time ON session_history(agent_id, timestamp);

            CREATE TABLE IF NOT EXISTS session_detail (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                topic TEXT,
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_sd_agent ON session_detail(agent_id);

            CREATE TABLE IF NOT EXISTS \"user\" (
                user_id TEXT PRIMARY KEY,
                username TEXT NOT NULL UNIQUE,
                data TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS user_profile (
                user_id TEXT PRIMARY KEY,
                data TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS role (
                role_id TEXT PRIMARY KEY,
                is_system INTEGER NOT NULL DEFAULT 0,
                data TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS api_key (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                key_hash TEXT NOT NULL UNIQUE,
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_apikey_user ON api_key(user_id);
            CREATE INDEX IF NOT EXISTS idx_apikey_hash ON api_key(key_hash);

            CREATE TABLE IF NOT EXISTS agent_config (
                agent_id TEXT PRIMARY KEY,
                data TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS agent_core (
                agent_id TEXT PRIMARY KEY,
                content TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS provider_config (
                variant TEXT PRIMARY KEY,
                data TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS global_config (
                key TEXT PRIMARY KEY,
                data TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS dream_journal (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                dream_cycle_id TEXT,
                stage TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_dj_agent ON dream_journal(agent_id);
            CREATE INDEX IF NOT EXISTS idx_dj_cycle ON dream_journal(dream_cycle_id);

            CREATE TABLE IF NOT EXISTS state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS session_file (
                id TEXT PRIMARY KEY,
                session_slug TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_sf_session ON session_file(session_slug, agent_id);
            ",
        )?;

        // FTS5 table for memory content search
        // We use content sync (external content) with triggers
        conn.execute_batch(
            "
            CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
                id UNINDEXED,
                title,
                content,
                tags,
                content='memory',
                content_rowid='rowid'
            );

            CREATE TRIGGER IF NOT EXISTS memory_ai AFTER INSERT ON memory BEGIN
                INSERT INTO memory_fts(rowid, id, title, content, tags)
                VALUES (new.rowid, new.id,
                    json_extract(new.data, '$.title'),
                    json_extract(new.data, '$.content'),
                    json_extract(new.data, '$.tags'));
            END;

            CREATE TRIGGER IF NOT EXISTS memory_ad AFTER DELETE ON memory BEGIN
                INSERT INTO memory_fts(memory_fts, rowid, id, title, content, tags)
                VALUES ('delete', old.rowid, old.id,
                    json_extract(old.data, '$.title'),
                    json_extract(old.data, '$.content'),
                    json_extract(old.data, '$.tags'));
            END;

            CREATE TRIGGER IF NOT EXISTS memory_au AFTER UPDATE ON memory BEGIN
                INSERT INTO memory_fts(memory_fts, rowid, id, title, content, tags)
                VALUES ('delete', old.rowid, old.id,
                    json_extract(old.data, '$.title'),
                    json_extract(old.data, '$.content'),
                    json_extract(old.data, '$.tags'));
                INSERT INTO memory_fts(rowid, id, title, content, tags)
                VALUES (new.rowid, new.id,
                    json_extract(new.data, '$.title'),
                    json_extract(new.data, '$.content'),
                    json_extract(new.data, '$.tags'));
            END;
            ",
        )?;

        init_memory_graph_schema(conn)?;
        init_revision_schema(conn)?;

        Ok(())
    }
}

impl VizierStorageProvider for SqliteStorage {}
