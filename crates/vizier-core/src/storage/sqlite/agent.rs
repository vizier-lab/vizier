use crate::time::Timestamp;
use anyhow::{Context, Result, anyhow};

use crate::storage::sql::{OptionalExtension, params};
use crate::{
    schema::{
        AgentConfig, CoreRevision, CoreRevisionSummary, PaginatedCoreRevisions, RevisionDiff,
        RevisionOrigin, RevisionTrigger, RollbackResponse,
    },
    storage::{
        agent::AgentStorage,
        diff::diff_lines,
        sqlite::{
            SqlStorage,
            core_revision::{self, CoreRevisionRow},
            memory_revision,
        },
    },
};

/// The rows of one agent that `delete_agent` used to leave behind: its conversations (history,
/// details, reactions, platform links, reply recordings), memory graph, passages and vectors,
/// tasks and their runs, dream journal, shares, and its keys in `state`. Each child row is
/// deleted explicitly, since the cascades only fire with `PRAGMA foreign_keys` on.
pub(crate) fn delete_agent_data(conn: &crate::storage::sql::Connection, agent_id: &str) -> Result<()> {
    let history = "SELECT uid FROM session_history WHERE agent_id = ?1";
    for child in ["message_reaction", "platform_message_link", "reply_audio"] {
        conn.execute(
            &format!("DELETE FROM {child} WHERE history_uid IN ({history})"),
            params![agent_id],
        )?;
    }
    for table in [
        "platform_message_link",
        "session_history",
        "session_detail",
        "task_run",
        "task",
        "dream_journal",
        "memory_passage",
        "memory_edge",
        "memory_node",
        "memory",
    ] {
        conn.execute(
            &format!("DELETE FROM {table} WHERE agent_id = ?1"),
            params![agent_id],
        )?;
    }
    conn.execute(
        "DELETE FROM shared_snapshot_audience WHERE share_id IN \
         (SELECT share_id FROM shared_snapshot WHERE agent_id = ?1)",
        params![agent_id],
    )?;
    conn.execute(
        "DELETE FROM shared_snapshot WHERE agent_id = ?1",
        params![agent_id],
    )?;

    // Its memory vectors, in every per-width table (`{agent}/{bundle}/{path}#{n}`).
    let vector_tables = match conn.backend() {
        crate::storage::sql::Backend::Sqlite => {
            "SELECT name FROM sqlite_master WHERE type = 'table' \
             AND sql LIKE 'CREATE VIRTUAL TABLE%' \
             AND (name = 'document_index' OR name LIKE 'document!_index!_d%' ESCAPE '!')"
        }
        crate::storage::sql::Backend::Postgres => {
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = current_schema() \
             AND table_name LIKE 'document!_index!_d%' ESCAPE '!'"
        }
    };
    let tables = conn
        .query_all(vector_tables, ())?
        .into_iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<crate::storage::sql::Result<Vec<_>>>()?;
    let prefix = format!("{agent_id}/");
    for table in tables {
        conn.execute(
            &format!(
                "DELETE FROM {table} WHERE context = 'memory' AND substr(path, 1, {}) = ?1",
                prefix.chars().count()
            ),
            params![prefix],
        )?;
    }

    // Its keys in `state`: dream bookkeeping, the memory index width, and the active topic of
    // each Discord/Telegram channel (`{agent}__{discord|telegram}__{id}`, ids without `_`).
    for key in [
        format!("dream_last_time:{agent_id}"),
        format!("dream_status:{agent_id}"),
        format!("memory.index_dimension.{agent_id}"),
    ] {
        conn.execute("DELETE FROM state WHERE key = ?1", params![key])?;
    }
    for platform in ["discord", "telegram"] {
        let prefix = format!("{agent_id}__{platform}__");
        let len = prefix.chars().count();
        conn.execute(
            &format!(
                "DELETE FROM state WHERE substr(key, 1, {len}) = ?1 \
                 AND substr(key, {}) NOT LIKE '%!_%' ESCAPE '!'",
                len + 1
            ),
            params![prefix],
        )?;
    }
    Ok(())
}

fn millis_to_datetime(ms: i64) -> Timestamp {
    crate::time::from_millis_opt(ms).unwrap_or_else(crate::time::now)
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
impl AgentStorage for SqlStorage {
    async fn list_agents(&self) -> Result<Vec<(String, AgentConfig)>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare("SELECT agent_id, data FROM agent_config")?;
        let rows = stmt
            .query_map([], |row| {
                let agent_id: String = row.get(0)?;
                let data: String = row.get(1)?;
                Ok((agent_id, data))
            })?
            .collect::<crate::storage::sql::Result<Vec<_>>>()?;
        drop(stmt);
        drop(conn);

        let mut agents = Vec::with_capacity(rows.len());
        for (agent_id, data) in rows {
            let Ok(config) = serde_json::from_str::<AgentConfig>(&data) else {
                continue;
            };
            let config =
                crate::encryption::fields::decrypt_agent_config(&self.cipher, &agent_id, &config)
                    .with_context(|| format!("agent '{agent_id}'"))?;
            agents.push((agent_id, config));
        }
        Ok(agents)
    }

    async fn get_agent(&self, agent_id: &str) -> Result<Option<AgentConfig>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare("SELECT data FROM agent_config WHERE agent_id = ?1")?;
        let mut rows = stmt.query_map(params![agent_id], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        let data = match rows.next() {
            Some(Ok(data)) => data,
            Some(Err(e)) => return Err(e.into()),
            None => return Ok(None),
        };
        drop(rows);
        drop(stmt);
        drop(conn);

        let config: AgentConfig = serde_json::from_str(&data)?;
        let config =
            crate::encryption::fields::decrypt_agent_config(&self.cipher, agent_id, &config)
                .with_context(|| format!("agent '{agent_id}'"))?;
        Ok(Some(config))
    }

    async fn create_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()> {
        let (protected, _) = crate::encryption::fields::protect_agent_config(
            &self.cipher,
            agent_id,
            config,
            crate::encryption::ProtectMode::Write,
        )?;
        let data = serde_json::to_string(&protected)?;
        let conn = self.conn.lock()?;
        conn.execute(
            "INSERT INTO agent_config (agent_id, data) VALUES (?1, ?2)",
            params![agent_id, data],
        )?;
        Ok(())
    }

    async fn update_agent(&self, agent_id: &str, config: &AgentConfig) -> Result<()> {
        let (protected, _) = crate::encryption::fields::protect_agent_config(
            &self.cipher,
            agent_id,
            config,
            crate::encryption::ProtectMode::Write,
        )?;
        let data = serde_json::to_string(&protected)?;
        let conn = self.conn.lock()?;
        let updated = conn.execute(
            "UPDATE agent_config SET data = ?1 WHERE agent_id = ?2",
            params![data, agent_id],
        )?;
        if updated == 0 {
            return Err(anyhow::anyhow!("Agent '{}' not found", agent_id));
        }
        Ok(())
    }

    async fn delete_agent(&self, agent_id: &str) -> Result<()> {
        let guard = self.conn.lock()?;
        // One transaction: an agent is deleted whole or not at all, never half.
        let conn = guard.unchecked_transaction()?;
        // Its conversations, memory, tasks, dreams, shares and per-agent state. These used to be
        // left behind, so a deleted agent's history outlived it (and came back with a re-created
        // agent of the same id).
        delete_agent_data(&conn, agent_id)?;
        conn.execute(
            "DELETE FROM agent_core WHERE agent_id = ?1",
            params![agent_id],
        )?;
        // History dies with the agent (FR-019): both revision tables are keyed by agent_id.
        core_revision::delete_agent(&conn, agent_id)?;
        memory_revision::delete_agent(&conn, agent_id)?;
        // Usage dies with the agent, and only with it (spec 018 FR-008).
        super::usage::delete_agent(&conn, agent_id)?;
        // Its bots go with it, and its attachments to shared integrations
        // (`IntegrationStorage::delete_agent_integrations`), on the connection already held.
        super::integration::delete_agent_integrations(&conn, agent_id)?;
        // What it asked the people it talked to.
        super::question::delete_agent(&conn, agent_id)?;
        conn.execute(
            "DELETE FROM client_tool_call WHERE agent_id = ?1",
            params![agent_id],
        )?;
        conn.execute(
            "DELETE FROM scratch_file WHERE agent_id = ?1",
            params![agent_id],
        )?;
        // Its detached scripts (spec 058); their tasks were stopped with the instance.
        conn.execute(
            "DELETE FROM detached_script WHERE agent_id = ?1",
            params![agent_id],
        )?;
        // Secret values and their change log never outlive their agent (spec 027 FR-005).
        super::secret::delete_agent_secrets(&conn, agent_id)?;
        // Its interactive pages and their versions (spec 063); the blobs go by prefix.
        super::ui_page::delete_agent_pages(&conn, agent_id)?;
        // Its custom tools, versions, categories and their log (spec 061).
        super::delete_agent_custom_tools(&conn, agent_id)?;
        // Its notifications go with it; their read rows cascade (spec 029).
        super::notification::delete_agent_notifications(&conn, agent_id)?;
        // Its projects go with it, before the config row they belong to (spec 040).
        super::project::delete_agent_projects(&conn, agent_id)?;
        conn.execute(
            "DELETE FROM session_turn WHERE agent_id = ?1",
            params![agent_id],
        )?;
        // Its ownership fence (spec 057): a re-created agent of the same id starts a new scope,
        // so this process also forgets the fence it held, which no row would match any more.
        conn.execute(
            "DELETE FROM owner_fence WHERE scope = ?1",
            params![format!("agent:{agent_id}")],
        )?;
        // Its Telegram side-chat deletion ledger (spec 033): message ids only, but still its own.
        conn.execute(
            "DELETE FROM state WHERE key = ?1",
            params![format!("side_chat_pending__{agent_id}")],
        )?;
        // Its transient session-file records and permanent artifact metadata go with it
        // (`specs/035-add-artifacts`); blobs and content files are removed after the commit by
        // `SessionFiles` and `Artifacts`.
        conn.execute(
            "DELETE FROM session_file WHERE agent_id = ?1",
            params![agent_id],
        )?;
        conn.execute(
            "DELETE FROM artifact_reference WHERE agent_id = ?1",
            params![agent_id],
        )?;
        conn.execute(
            "DELETE FROM artifact_event WHERE agent_id = ?1",
            params![agent_id],
        )?;
        conn.execute(
            "DELETE FROM artifact WHERE agent_id = ?1",
            params![agent_id],
        )?;
        let deleted = conn.execute(
            "DELETE FROM agent_config WHERE agent_id = ?1",
            params![agent_id],
        )?;
        if deleted == 0 {
            // Dropping the transaction rolls every statement above back.
            return Err(anyhow::anyhow!("Agent '{}' not found", agent_id));
        }
        conn.commit()?;
        self.fences
            .forget(&crate::storage::fence::Scope::Agent(agent_id.to_owned()));
        Ok(())
    }

    /// **Deprecated** (specs/043-unify-core-and-memory, FR-034): identity is the memory
    /// document `core/soul`. This answers from the legacy sources only, for the spawn-time
    /// seed and for the removal tracked in issue #82.
    async fn get_agent_core(&self, agent_id: &str) -> Result<Option<String>> {
        if let Some(content) = self.read_agent_core(agent_id)? {
            return Ok(Some(content));
        }
        // Legacy location: before `agent_core` was wired up, CORE lived inside the agent config
        // JSON. `VizierDependencies::migrate_agent_cores` moves it over on startup; reading it
        // here keeps a not-yet-migrated deployment intact in the meantime.
        Ok(self.get_agent(agent_id).await?.and_then(|c| c.core))
    }

    /// **Deprecated** (specs/043-unify-core-and-memory, FR-034). Kept for the filesystem
    /// migration, which still carries a legacy CORE into the legacy table; nothing else writes
    /// it. Identity saves go through `write_memory` at `core/soul`.
    ///
    /// Upserts the CORE and appends a `core_revision` entry in one transaction, so a save and
    /// its history entry either both land or neither does (research Decision 8).
    async fn set_agent_core(
        &self,
        agent_id: &str,
        core: &str,
        origin: &RevisionOrigin,
    ) -> Result<()> {
        let conn = self.conn.lock()?;
        let tx = conn.unchecked_transaction()?;
        let current: Option<String> = tx
            .query_row(
                "SELECT content FROM agent_core WHERE agent_id = ?1",
                params![agent_id],
                |row| row.get(0),
            )
            .optional()?;
        tx.execute(
            "INSERT INTO agent_core (agent_id, content) VALUES (?1, ?2)
             ON CONFLICT(agent_id) DO UPDATE SET content = excluded.content",
            params![agent_id, core],
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
        let conn = self.conn.lock()?;
        // A CORE that pre-dates history gets its baseline on first listing (FR-018), so the
        // list is never empty for an agent that has a CORE.
        let current: Option<String> = conn
            .query_row(
                "SELECT content FROM agent_core WHERE agent_id = ?1",
                params![agent_id],
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
        let conn = self.conn.lock()?;
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
        let conn = self.conn.lock()?;
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
            let conn = self.conn.lock()?;
            let target = core_revision::get(&conn, agent_id, seq)?
                .ok_or_else(|| anyhow!("unknown version {seq}"))?;
            let before = core_revision::latest(&conn, agent_id)?.map(|r| r.seq);
            (target, before)
        };
        // A rollback is just a save with rollback provenance — never a history rewrite.
        let origin = origin
            .clone()
            .with_trigger(RevisionTrigger::Rollback { restored_from: seq });
        self.set_agent_core(agent_id, &target.content, &origin)
            .await?;
        let after = {
            let conn = self.conn.lock()?;
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

impl SqlStorage {
    fn read_agent_core(&self, agent_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock()?;
        Ok(conn
            .query_row(
                "SELECT content FROM agent_core WHERE agent_id = ?1",
                params![agent_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?)
    }
}

#[cfg(test)]
mod encryption_tests {
    use serde_json::json;

    use super::*;
    use crate::encryption::storage_tests::{agent_config, file_backed, raw_value};

    /// Shell env values are protected at rest; model, prompt, shell path and `core` stay
    /// readable, and a tampered row fails instead of disappearing from the list.
    #[tokio::test]
    async fn shell_env_is_protected_but_the_rest_of_the_config_is_readable() {
        let h = file_backed();
        let s = &h.storage;
        s.create_agent(
            "dp-env",
            &agent_config(json!({"SHELL_TOKEN": "shell-secret-abc123"})),
        )
        .await
        .unwrap();

        let raw = raw_value(
            s,
            "SELECT data FROM agent_config WHERE agent_id = 'dp-env'",
            &[],
        );
        assert!(raw.contains("enc:v1:"), "{raw}");
        assert!(!raw.contains("shell-secret-abc123"));
        assert!(raw.contains("\"model\":\"dummyplug\""));
        assert!(raw.contains("\"system_prompt\":\"be helpful\""));
        assert!(raw.contains("/bin/sh"));
        assert!(raw.contains("\"core\":\"core survives\""));

        let config = s.get_agent("dp-env").await.unwrap().unwrap();
        assert_eq!(config.core.as_deref(), Some("core survives"));

        s.conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_config SET data = replace(data, 'enc:v1:', 'enc:v1:x')",
                [],
            )
            .unwrap();
        assert!(s.get_agent("dp-env").await.is_err());
        assert!(s.list_agents().await.is_err());
    }

    /// Re-serializing the current `AgentConfig` drops the pre-016 moved keys, so their dead
    /// credentials cannot survive a save.
    #[tokio::test]
    async fn saving_an_agent_drops_legacy_credential_keys() {
        let h = file_backed();
        let s = &h.storage;
        let mut config = agent_config(json!({"SHELL_TOKEN": "shell-secret-abc123"}));
        config.core = Some("core".into());
        s.create_agent("dp-env", &config).await.unwrap();

        let raw = {
            let conn = s.conn.lock().unwrap();
            conn.query_row(
                "SELECT data FROM agent_config WHERE agent_id = 'dp-env'",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap()
        };
        assert!(!raw.contains("discord_token"));
        assert!(!raw.contains("telegram_token"));
        assert!(!raw.contains("mcp_servers"));
    }

    crate::postgres_tests!(
        shell_env_is_protected_but_the_rest_of_the_config_is_readable,
        saving_an_agent_drops_legacy_credential_keys,
    );
}

#[cfg(test)]
mod delete_tests {
    use std::sync::Arc;

    use super::*;
    use crate::storage::sql::SqlStore;

    fn storage() -> SqlStorage {
        // Auto-loaded into connections opened after this, for the memory vector tables.
        SqlStorage::register_sqlite_vec();
        let store = SqlStore::sqlite_in_memory().unwrap();
        store
            .lock()
            .unwrap()
            .with_rusqlite(|c| {
                c.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
                SqlStorage::init_schema(c).unwrap();
            })
            .unwrap();
        let dir = tempfile::tempdir().unwrap().keep();
        SqlStorage::new(store)
    }

    fn agent() -> AgentConfig {
        serde_json::from_value(serde_json::json!({
            "name": "A",
            "provider": "dummyplug",
            "model": "dummyplug",
            "thinking_depth": 1,
            "tools": { "timeout": "30s" },
            "silent_read_initiative_chance": 0.0,
            "prompt_timeout": "30s",
            "heartbeat_interval": "60s",
        }))
        .unwrap()
    }

    /// Rows of every kind `delete_agent` once left behind, for `agent`.
    fn seed(s: &SqlStorage, agent: &str) {
        let conn = s.conn.lock().unwrap();
        let uid = format!("{agent}-h1");
        let run = [
            format!("INSERT INTO session_history (uid, agent_id, channel, topic, timestamp, content_type, data) VALUES ('{uid}', '{agent}', 'http__u__c', 't', 1, 'request', '{{}}')"),
            format!("INSERT INTO message_reaction (history_uid, reactor_id, emoji, added_at) VALUES ('{uid}', 'p', 'x', 1)"),
            format!("INSERT INTO platform_message_link (agent_id, platform, chat_id, message_id, history_uid) VALUES ('{agent}', 'discord', '1', '2', '{uid}')"),
            format!("INSERT INTO session_detail (id, agent_id, channel, topic, data) VALUES ('{agent}-d', '{agent}', 'http__u__c', 't', '{{}}')"),
            format!("INSERT INTO task (id, agent_id, slug, data) VALUES ('{agent}-t', '{agent}', 's', '{{}}')"),
            format!("INSERT INTO task_run (agent_id, task_slug, ran_at, session_key, state) VALUES ('{agent}', 's', 1, 'k', 'answered')"),
            format!("INSERT INTO dream_journal (id, agent_id, stage, timestamp, data) VALUES ('{agent}-j', '{agent}', 'x', 1, '{{}}')"),
            format!("INSERT INTO memory_node (agent_id, bundle, path, slug, title, tags_json, created_at, updated_at) VALUES ('{agent}', 'b', 'p', 'p', 'P', '[]', '', '')"),
            format!("INSERT INTO state (key, value) VALUES ('dream_status:{agent}', '\"Idle\"')"),
            format!("INSERT INTO state (key, value) VALUES ('{agent}__discord__42', '{{}}')"),
        ];
        for sql in run {
            conn.execute(&sql, ()).unwrap();
        }
    }

    fn count(s: &SqlStorage, sql: &str) -> i64 {
        let conn = s.conn.lock().unwrap();
        conn.query_row(sql, (), |row| row.get(0)).unwrap()
    }

    #[tokio::test]
    async fn deleting_an_agent_deletes_its_conversations_and_everything_else() {
        let s = storage();
        s.create_agent("a", &agent()).await.unwrap();
        s.create_agent("a__discord", &agent()).await.unwrap();
        s.create_agent("b", &agent()).await.unwrap();
        seed(&s, "a");
        seed(&s, "b");
        // Another agent's channel state whose key starts like one of `a`'s.
        s.conn
            .lock()
            .unwrap()
            .execute("INSERT INTO state (key, value) VALUES ('a__discord__discord__7', '{}')", ())
            .unwrap();

        {
            let conn = s.conn.lock().unwrap();
            conn.execute_batch(
                "CREATE VIRTUAL TABLE document_index USING vec0(embedding float[2], context TEXT, path TEXT);
                 CREATE VIRTUAL TABLE document_index_d3 USING vec0(embedding float[3], context TEXT, path TEXT);",
            )
            .unwrap();
            for (table, dim, path) in [
                ("document_index", 2, "a/b/p#0"),
                ("document_index", 2, "b/b/p#0"),
                ("document_index_d3", 3, "a/b/q#0"),
                ("document_index_d3", 3, "a__discord/b/q#0"),
            ] {
                let v = format!("[{}]", vec!["0.5"; dim].join(","));
                conn.execute(
                    &format!("INSERT INTO {table} (embedding, context, path) VALUES (?1, 'memory', ?2)"),
                    params![v, path],
                )
                .unwrap();
            }
        }

        s.delete_agent("a").await.unwrap();

        assert_eq!(count(&s, "SELECT COUNT(*) FROM document_index WHERE path LIKE 'a/%'"), 0);
        assert_eq!(count(&s, "SELECT COUNT(*) FROM document_index_d3 WHERE path LIKE 'a/%'"), 0);
        assert_eq!(count(&s, "SELECT COUNT(*) FROM document_index"), 1);
        assert_eq!(count(&s, "SELECT COUNT(*) FROM document_index_d3"), 1);

        for table in [
            "session_history",
            "session_detail",
            "platform_message_link",
            "task",
            "task_run",
            "dream_journal",
            "memory_node",
        ] {
            assert_eq!(
                count(&s, &format!("SELECT COUNT(*) FROM {table} WHERE agent_id = 'a'")),
                0,
                "{table} outlived its agent"
            );
            assert_eq!(
                count(&s, &format!("SELECT COUNT(*) FROM {table} WHERE agent_id = 'b'")),
                1,
                "{table} of another agent was touched"
            );
        }
        assert_eq!(count(&s, "SELECT COUNT(*) FROM message_reaction"), 1);
        assert_eq!(count(&s, "SELECT COUNT(*) FROM state WHERE key IN ('dream_status:a', 'a__discord__42')"), 0);
        assert_eq!(count(&s, "SELECT COUNT(*) FROM state WHERE key IN ('dream_status:b', 'b__discord__42', 'a__discord__discord__7')"), 3);
    }

    #[tokio::test]
    async fn deleting_an_unknown_agent_deletes_nothing() {
        let s = storage();
        seed(&s, "ghost");
        assert!(s.delete_agent("ghost").await.is_err());
        assert_eq!(count(&s, "SELECT COUNT(*) FROM session_history WHERE agent_id = 'ghost'"), 1);
    }
}
