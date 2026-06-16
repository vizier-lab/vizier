use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::Connection;

use crate::storage::VizierStorageProvider;
use crate::utils::build_path;

mod agent;
mod dream_journal;
mod global_config;
mod history;
mod memory;
mod provider;
mod session;
mod session_file;
mod state;
mod task;
mod user;

#[derive(Clone)]
pub struct SqliteStorage {
    pub conn: Arc<Mutex<Connection>>,
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

    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
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

        Ok(())
    }
}

impl VizierStorageProvider for SqliteStorage {}
