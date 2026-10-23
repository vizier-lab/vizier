use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::Connection;

use crate::error::VizierError;
use crate::storage::VizierStorageProvider;
use crate::storage::document::DocumentStore;
use crate::utils::build_path;

mod agent;
mod background_job;
mod catalogue;
mod client_call;
pub(crate) mod core_revision;
mod dream_journal;
mod global_config;
mod history;
mod integration;
mod memory;
pub(crate) mod memory_revision;
mod provider;
mod reaction;
mod scratch;
mod session;
mod session_file;
mod state;
mod task;
mod task_run;
mod turn;
pub(crate) mod usage;
mod user;

#[derive(Clone)]
pub struct SqliteStorage {
    pub conn: Arc<Mutex<Connection>>,
    pub document_store: Arc<dyn DocumentStore>,
}

/// The Memory Graph Index tables (`memory_node`/`memory_edge`, data-model.md) plus the derived
/// passage coordinate table (`memory_passage`,
/// `specs/009-memory-semantic-chunking/data-model.md` §3), split out from `init_schema` so unit
/// tests (`src/storage/memory_bundle.rs`) can stand up just these tables against an in-memory
/// connection without the rest of the application schema.
///
/// Everything here is derived and rebuildable from the concept documents on disk. For
/// `memory_passage` specifically, the *absence* of rows for a document is the only marker that it
/// has not been chunked yet — there is no separate progress table (research Decision 8).
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

        CREATE TABLE IF NOT EXISTS memory_passage (
            agent_id TEXT NOT NULL,
            bundle TEXT NOT NULL,
            path TEXT NOT NULL,
            ordinal INTEGER NOT NULL,
            line_start INTEGER NOT NULL,
            line_end INTEGER NOT NULL,
            char_start INTEGER NOT NULL,
            char_end INTEGER NOT NULL,
            continues INTEGER NOT NULL DEFAULT 0,
            content_hash TEXT NOT NULL,
            PRIMARY KEY (agent_id, bundle, path, ordinal)
        );
        CREATE INDEX IF NOT EXISTS idx_memory_passage_doc ON memory_passage(agent_id, bundle, path);
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

            CREATE TABLE IF NOT EXISTS custom_provider (
                id TEXT PRIMARY KEY,
                data TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS catalogue_snapshot (
                id          INTEGER PRIMARY KEY CHECK (id = 1),
                schema      INTEGER NOT NULL,
                source_url  TEXT NOT NULL,
                etag        TEXT,
                fetched_at  INTEGER NOT NULL,
                checked_at  INTEGER NOT NULL,
                last_error  TEXT,
                failed_at   INTEGER,
                data        BLOB NOT NULL
            );

            CREATE TABLE IF NOT EXISTS catalogue_logo (
                id          TEXT PRIMARY KEY,
                svg         BLOB,
                fetched_at  INTEGER NOT NULL
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

        init_history_schema(conn)?;
        init_reaction_schema(conn)?;
        init_memory_graph_schema(conn)?;
        init_revision_schema(conn)?;
        init_task_run_schema(conn)?;
        init_background_job_schema(conn)?;
        init_integration_schema(conn)?;
        init_usage_schema(conn)?;
        init_client_call_schema(conn)?;
        init_scratch_schema(conn)?;
        init_turn_schema(conn)?;

        Ok(())
    }
}

/// The `client_tool_call` table (`specs/019-client-tools/data-model.md`): the outcome and
/// saved state of each client tool call, keyed by its history entry's uid. Split out like
/// [`init_task_run_schema`] so the tests in `src/storage/sqlite/client_call.rs` can stand it
/// up against an in-memory connection.
pub fn init_client_call_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS client_tool_call (
            history_uid      TEXT PRIMARY KEY,
            agent_id         TEXT NOT NULL,
            channel          TEXT NOT NULL,
            topic            TEXT,
            status           TEXT NOT NULL,
            detail           TEXT,
            state            TEXT,
            state_updated_at TEXT,
            created_at       TEXT NOT NULL,
            finished_at      TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_client_call_session ON client_tool_call(agent_id, channel, topic);
        CREATE INDEX IF NOT EXISTS idx_client_call_pending ON client_tool_call(status) WHERE status = 'pending';
        ",
    )?;
    Ok(())
}

/// The `scratch_file` table (`specs/020-scratchpad-tools/data-model.md` §1): an agent's
/// temporary notes, private to one conversation and cleared once it goes idle. `topic` is `''`
/// for a session without one, so the upsert's `ON CONFLICT` sees one key (research R2). Split
/// out so the tests in `src/storage/sqlite/scratch.rs` can stand it up against an in-memory
/// connection.
pub fn init_scratch_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS scratch_file (
            agent_id   TEXT    NOT NULL,
            channel    TEXT    NOT NULL,
            topic      TEXT    NOT NULL DEFAULT '',
            name       TEXT    NOT NULL,
            content    TEXT    NOT NULL,
            size       INTEGER NOT NULL,
            updated_at TEXT    NOT NULL,
            PRIMARY KEY (agent_id, channel, topic, name)
        );
        ",
    )?;
    Ok(())
}

/// Accepted work that has not finished (`specs/023-persist-turn-activity/data-model.md`).
pub fn init_turn_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS session_turn (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id        TEXT    NOT NULL,
            channel         TEXT    NOT NULL,
            topic           TEXT    NOT NULL DEFAULT '',
            session         TEXT    NOT NULL,
            request         TEXT    NOT NULL,
            state           TEXT    NOT NULL,
            request_uid     TEXT,
            accepted_at     TEXT    NOT NULL,
            started_at      TEXT,
            resume_count    INTEGER NOT NULL DEFAULT 0,
            steps_at_resume INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_session_turn_agent ON session_turn (agent_id, id);
        ",
    )?;
    Ok(())
}

/// The usage accounting tables (`specs/018-usage-accounting/data-model.md` §2), split out like
/// [`init_task_run_schema`] so the tests in `src/storage/sqlite/usage.rs` can stand them up
/// against an in-memory connection. Rows are only ever inserted, and deleted with their agent.
/// A turn is failed iff its `turn_id` is in `usage_failed_turn`: a separate keyed row, so the
/// failure mark and the turn's last insert can land in either order (research R3).
pub fn init_usage_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS usage_event (
            id                          INTEGER PRIMARY KEY,
            agent_id                    TEXT    NOT NULL,
            at                          INTEGER NOT NULL,
            activity                    TEXT    NOT NULL,
            channel                     TEXT,
            topic                       TEXT,
            place                       TEXT    NOT NULL,
            turn_id                     TEXT,
            round                       INTEGER,
            provider                    TEXT    NOT NULL DEFAULT '',
            model                       TEXT    NOT NULL DEFAULT '',
            served_model                TEXT,
            input_tokens                INTEGER NOT NULL DEFAULT 0,
            cached_input_tokens         INTEGER NOT NULL DEFAULT 0,
            cache_creation_input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens               INTEGER NOT NULL DEFAULT 0,
            duration_ms                 INTEGER NOT NULL DEFAULT 0,
            legacy                      INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS usage_event_agent_at   ON usage_event (agent_id, at);
        CREATE INDEX IF NOT EXISTS usage_event_agent_conv ON usage_event (agent_id, channel, topic);
        CREATE INDEX IF NOT EXISTS usage_event_turn       ON usage_event (turn_id) WHERE turn_id IS NOT NULL;

        CREATE TABLE IF NOT EXISTS usage_failed_turn (
            turn_id  TEXT    PRIMARY KEY,
            agent_id TEXT    NOT NULL,
            at       INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS usage_failed_turn_agent ON usage_failed_turn (agent_id);
        ",
    )?;

    // An HTTP place was briefly `http:{user}:{topic}`, which loses which HTTP channel it was;
    // it is `http:{channel slug}:{topic}`. Rewrites the earlier form, and matches nothing once
    // every row has the current one.
    conn.execute(
        "UPDATE usage_event SET place = 'http:' || channel || ':' || COALESCE(topic, '') \
         WHERE channel LIKE 'http\\_\\_%' ESCAPE '\\' \
           AND place <> 'http:' || channel || ':' || COALESCE(topic, '')",
        [],
    )?;

    Ok(())
}

/// The `task_run` table (`specs/011-task-completion-reports/data-model.md`), split out of
/// `init_schema` like [`init_history_schema`] so the cursor and invariant tests in
/// `src/storage/sqlite/task_run.rs` can stand up just this table against an in-memory
/// connection.
///
/// `AUTOINCREMENT` matters: `id` is the low half of the run-list page cursor and must be
/// monotonic, which a plain `INTEGER PRIMARY KEY` is not — it can reuse a rowid freed by a
/// delete, and the cursor would then skip or duplicate a run.
pub fn init_task_run_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS task_run (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id     TEXT NOT NULL,
            task_slug    TEXT NOT NULL,
            ran_at       INTEGER NOT NULL,
            finished_at  INTEGER,
            session_key  TEXT NOT NULL,
            state        TEXT NOT NULL
        );
        -- the run list: newest-first within one task, the only ordering anything asks for
        CREATE INDEX IF NOT EXISTS idx_task_run_task ON task_run(agent_id, task_slug, ran_at DESC, id DESC);
        -- the startup sweep, and the overlap check
        CREATE INDEX IF NOT EXISTS idx_task_run_state ON task_run(state);
        ",
    )?;

    Ok(())
}

/// The integration tables (`specs/016-integrations-section/data-model.md` §1–2, and
/// `specs/017-agent-self-integrations/data-model.md` §3–4 for grants and events), split out like
/// [`init_task_run_schema`] so the tests in `src/storage/sqlite/integration.rs` can stand them up
/// against an in-memory connection.
///
/// `name` is deliberately not unique: two MCP integrations may share one, and only an agent
/// may not have both attached (research R12), which `IntegrationStorage` enforces.
pub fn init_integration_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS integration (
            id          TEXT PRIMARY KEY,
            kind        TEXT NOT NULL,
            family      TEXT NOT NULL,
            name        TEXT NOT NULL,
            settings    TEXT NOT NULL,
            enabled     INTEGER NOT NULL,
            scope       TEXT NOT NULL,
            created_by  TEXT,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS integration_attachment (
            integration_id TEXT NOT NULL REFERENCES integration(id) ON DELETE CASCADE,
            agent_id       TEXT NOT NULL,
            created_at     TEXT NOT NULL,
            PRIMARY KEY (integration_id, agent_id)
        );
        CREATE INDEX IF NOT EXISTS idx_integration_attachment_agent ON integration_attachment(agent_id);

        CREATE TABLE IF NOT EXISTS integration_grant (
            agent_id    TEXT NOT NULL,
            family      TEXT NOT NULL,
            granted_by  TEXT NOT NULL,
            granted_at  TEXT NOT NULL,
            PRIMARY KEY (agent_id, family)
        );

        CREATE TABLE IF NOT EXISTS integration_event (
            id               TEXT PRIMARY KEY,
            agent_id         TEXT NOT NULL,
            integration_id   TEXT,
            integration_name TEXT NOT NULL,
            family           TEXT NOT NULL,
            actor            TEXT NOT NULL,
            action           TEXT NOT NULL,
            fields           TEXT NOT NULL DEFAULT '[]',
            session          TEXT,
            at               TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_integration_event_agent ON integration_event(agent_id, at DESC);
        CREATE INDEX IF NOT EXISTS idx_integration_event_integration ON integration_event(integration_id, at DESC);
        ",
    )?;
    // specs/017-agent-self-integrations data-model §1: who made it, as a `Requester`.
    add_column_if_missing(conn, "integration", "creator", "TEXT").map_err(|e| anyhow::anyhow!(e.0))?;

    Ok(())
}

/// The background-job tables (`specs/012-background-subagent-results/data-model.md`), split
/// out like [`init_task_run_schema`] so the tests in `src/storage/sqlite/background_job.rs` can
/// stand them up against an in-memory connection.
///
/// There is no result column: an answered piece's text is the last message in its own
/// session's history.
pub fn init_background_job_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS background_job (
            id             TEXT PRIMARY KEY,
            kind           TEXT NOT NULL,
            origin_agent   TEXT NOT NULL,
            origin_channel TEXT NOT NULL,
            origin_topic   TEXT,
            depth          INTEGER NOT NULL,
            timeout_secs   INTEGER NOT NULL,
            created_at     INTEGER NOT NULL,
            finished_at    INTEGER,
            state          TEXT NOT NULL,
            cancelled_by   TEXT,
            reason         TEXT
        );
        -- the tray read, the topic-list badge, and the cancel cascade
        CREATE INDEX IF NOT EXISTS idx_bg_job_origin ON background_job(origin_agent, origin_channel, origin_topic, state);
        -- list_background_jobs
        CREATE INDEX IF NOT EXISTS idx_bg_job_agent ON background_job(origin_agent, state);
        -- the startup sweep
        CREATE INDEX IF NOT EXISTS idx_bg_job_state ON background_job(state);

        CREATE TABLE IF NOT EXISTS background_piece (
            job_id          TEXT NOT NULL REFERENCES background_job(id) ON DELETE CASCADE,
            ordinal         INTEGER NOT NULL,
            prompt          TEXT NOT NULL,
            executor_agent  TEXT NOT NULL,
            session_channel TEXT NOT NULL,
            session_topic   TEXT NOT NULL,
            started_at      INTEGER NOT NULL,
            finished_at     INTEGER,
            state           TEXT NOT NULL,
            reason          TEXT,
            PRIMARY KEY (job_id, ordinal)
        );
        CREATE INDEX IF NOT EXISTS idx_bg_piece_state ON background_piece(state);
        ",
    )?;

    Ok(())
}

/// The `session_history` table, split out of `init_schema` like
/// [`init_memory_graph_schema`] so the ordering tests in `src/storage/sqlite/history.rs` can
/// stand up just this table against an in-memory connection.
pub fn init_history_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS session_history (
            uid TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            channel TEXT NOT NULL,
            topic TEXT,
            timestamp INTEGER NOT NULL,
            content_type TEXT NOT NULL,
            data TEXT NOT NULL,
            seq INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_sh_session ON session_history(agent_id, channel, topic);
        CREATE INDEX IF NOT EXISTS idx_sh_time ON session_history(timestamp);
        CREATE INDEX IF NOT EXISTS idx_sh_agent_time ON session_history(agent_id, timestamp);
        ",
    )?;

    // `seq` postdates the table, so a database created by an earlier build has
    // `session_history` without the column and `CREATE TABLE IF NOT EXISTS` will not add it.
    // The column has to exist before `idx_sh_seq` can reference it.
    add_column_if_missing(conn, "session_history", "seq", "INTEGER")?;
    conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_sh_seq ON session_history(seq);")?;

    Ok(())
}

/// The reaction tables (`specs/013-reaction-awareness/data-model.md`), split out like
/// [`init_history_schema`] so the tests in `src/storage/sqlite/reaction.rs` can stand them up
/// against an in-memory connection. Must run after `init_history_schema`, which owns the
/// table both cascade from.
///
/// Also moves reactions an earlier build kept inside each history row's `data` blob into
/// `message_reaction`, clearing the blob's array. It needs no marker: after one run no row
/// has a non-empty array left, so a second run finds nothing to do.
pub fn init_reaction_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS message_reaction (
            history_uid  TEXT NOT NULL REFERENCES session_history(uid) ON DELETE CASCADE,
            reactor_id   TEXT NOT NULL,
            reactor_name TEXT,
            emoji        TEXT NOT NULL,
            added_at     INTEGER NOT NULL,
            PRIMARY KEY (history_uid, reactor_id, emoji)
        );
        CREATE INDEX IF NOT EXISTS idx_reaction_uid ON message_reaction(history_uid);

        CREATE TABLE IF NOT EXISTS platform_message_link (
            agent_id    TEXT NOT NULL,
            platform    TEXT NOT NULL,
            chat_id     TEXT NOT NULL,
            message_id  TEXT NOT NULL,
            history_uid TEXT NOT NULL REFERENCES session_history(uid) ON DELETE CASCADE,
            PRIMARY KEY (agent_id, platform, chat_id, message_id)
        );
        ",
    )?;

    let legacy: Vec<(String, i64, String)> = {
        let mut stmt = conn.prepare(
            "SELECT uid, timestamp, data FROM session_history
             WHERE json_array_length(json_extract(data, '$.reactions')) > 0",
        )?;
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    if legacy.is_empty() {
        return Ok(());
    }

    let tx = conn.unchecked_transaction()?;
    for (uid, timestamp, data) in legacy {
        let mut entry: crate::schema::SessionHistory = match serde_json::from_str(&data) {
            Ok(entry) => entry,
            Err(err) => {
                tracing::warn!("skipping reaction migration for history entry {uid}: {err}");
                continue;
            }
        };
        for reaction in entry.reactions.drain(..) {
            tx.execute(
                "INSERT OR IGNORE INTO message_reaction
                    (history_uid, reactor_id, reactor_name, emoji, added_at)
                 VALUES (?1, ?2, NULL, ?3, ?4)",
                rusqlite::params![uid, reaction.user_id, reaction.emoji, timestamp],
            )?;
        }
        tx.execute(
            "UPDATE session_history SET data = ?1 WHERE uid = ?2",
            rusqlite::params![serde_json::to_string(&entry)?, uid],
        )?;
    }
    tx.commit()?;

    Ok(())
}

/// Add `column` to `table` when it is not there already, leaving it alone when it is.
///
/// The project's whole schema is one batch of `CREATE TABLE IF NOT EXISTS`, which by
/// construction does nothing to a table that already exists — so a column added after a
/// table shipped needs an explicit `ALTER`. The `PRAGMA table_info` guard is preferred over
/// running the `ALTER` and ignoring its error, because "duplicate column name" is the only
/// failure that would be benign to ignore and swallowing it would hide every other one
/// (`specs/010-webui-reasoning-display/research.md` Decision 2).
pub fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    decl: &str,
) -> crate::Result<()> {
    let present = {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .map_err(|err| VizierError(format!("failed to read {table} columns: {err}")))?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|err| VizierError(format!("failed to read {table} columns: {err}")))?;
        let mut present = false;
        for name in names {
            let name =
                name.map_err(|err| VizierError(format!("failed to read {table} columns: {err}")))?;
            if name == column {
                present = true;
                break;
            }
        }
        present
    };

    if present {
        return Ok(());
    }

    conn.execute_batch(&format!(
        "ALTER TABLE \"{table}\" ADD COLUMN \"{column}\" {decl};"
    ))
    .map_err(|err| VizierError(format!("failed to add {table}.{column}: {err}")))?;
    tracing::info!("added column {table}.{column}");

    Ok(())
}

impl VizierStorageProvider for SqliteStorage {}
