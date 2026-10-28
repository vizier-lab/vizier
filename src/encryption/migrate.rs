//! The one-time plaintext-to-protected conversion (`contracts/migration.md`).
//!
//! It runs inside `VizierDependencies::new`, after the storage-shape migrations and before the
//! main storage (and therefore any agent, channel reader or scheduler) exists. It is idempotent:
//! a valid envelope is validated and skipped, plaintext is encrypted, and the marker is written
//! only after the transaction, a WAL truncation and `VACUUM` have succeeded.
//!
//! ## Tracked for removal in #82
//!
//! [`migrate_secrets_to_encrypted`], [`MIGRATION_MARKER`], the plaintext branch
//! ([`crate::encryption::SecretCipher::open`]), the pre-016 agent-key scrub below and their
//! tests are one removable unit. The cipher, key sources, key id, AAD and envelope reader are
//! permanent and must not be removed with it.

use anyhow::{Result, anyhow, bail};
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;

use crate::{
    encryption::{
        IntegrationSecretPaths, ProtectMode, SecretCipher,
        fields::{
            protect_agent_config, protect_agent_secret, protect_custom_provider,
            protect_global_config, protect_integration_settings, protect_provider_entry,
        },
    },
    schema::{AgentConfig, CustomProvider, GlobalConfigEntry, ProviderEntry},
};

/// The durable marker that the one-time conversion completed. Removed with the migration after
/// the last pre-1.0 release.
pub const MIGRATION_MARKER: &str = "migration.secrets_encrypted_v1";

/// How many rows each table converted. Logged per table, never with a value.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ConversionCount {
    pub agent_secret: usize,
    pub provider_config: usize,
    pub custom_provider: usize,
    pub integration: usize,
    pub agent_config: usize,
    pub global_config: usize,
}

impl ConversionCount {
    pub fn total(&self) -> usize {
        self.agent_secret
            + self.provider_config
            + self.custom_provider
            + self.integration
            + self.agent_config
            + self.global_config
    }
}

/// Convert every in-scope plaintext value, reclaim the old bytes, then record the marker.
///
/// A failure before the marker leaves the marker absent; the next start retries. Values already
/// protected by this key are validated and skipped, so an interrupted run resumes safely.
pub fn migrate_secrets_to_encrypted(
    conn: &Connection,
    cipher: &SecretCipher,
    paths: &dyn IntegrationSecretPaths,
) -> Result<ConversionCount> {
    if !cipher.is_enabled() {
        bail!("the encryption migration requires an enabled encryption key");
    }
    if marker_present(conn)? {
        return Ok(ConversionCount::default());
    }

    // Old plaintext must not linger in freed pages after the UPDATEs.
    conn.execute_batch("PRAGMA secure_delete=ON;")
        .map_err(reclaim_error)?;

    let mut counts = ConversionCount::default();
    {
        let tx = conn.unchecked_transaction()?;
        counts.agent_secret = convert_agent_secrets(&tx, cipher)?;
        counts.provider_config = convert_provider_config(&tx, cipher)?;
        counts.custom_provider = convert_custom_provider(&tx, cipher)?;
        counts.integration = convert_integrations(&tx, cipher, paths)?;
        counts.agent_config = convert_agent_configs(&tx, cipher)?;
        counts.global_config = convert_global_config(&tx, cipher)?;
        tx.commit()?;
    }

    reclaim_plaintext(conn)?;
    write_marker(conn)?;

    tracing::info!(
        agent_secret = counts.agent_secret,
        provider_config = counts.provider_config,
        custom_provider = counts.custom_provider,
        integration = counts.integration,
        agent_config = counts.agent_config,
        global_config = counts.global_config,
        total = counts.total(),
        "converted plaintext secrets to encrypted storage"
    );
    Ok(counts)
}

/// Whether the one-time conversion already completed.
pub fn marker_present(conn: &Connection) -> Result<bool> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM state WHERE key = ?1",
            rusqlite::params![MIGRATION_MARKER],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value.is_some())
}

fn write_marker(conn: &Connection) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO state (key, value) VALUES (?1, 'true')",
        rusqlite::params![MIGRATION_MARKER],
    )?;
    Ok(())
}

fn convert_agent_secrets(conn: &Connection, cipher: &SecretCipher) -> Result<usize> {
    let rows: Vec<(String, String, String)> = {
        let mut stmt =
            conn.prepare("SELECT agent_id, name, value FROM agent_secret ORDER BY agent_id, name")?;
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut converted = 0;
    for (agent_id, name, value) in rows {
        let (stored, changed) =
            protect_agent_secret(cipher, &agent_id, &name, &value, ProtectMode::Migration)?;
        if changed {
            conn.execute(
                "UPDATE agent_secret SET value = ?1 WHERE agent_id = ?2 AND name = ?3",
                rusqlite::params![stored, agent_id, name],
            )?;
            converted += 1;
        }
    }
    Ok(converted)
}

fn convert_provider_config(conn: &Connection, cipher: &SecretCipher) -> Result<usize> {
    let rows: Vec<(String, String)> = {
        let mut stmt =
            conn.prepare("SELECT variant, data FROM provider_config ORDER BY variant")?;
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut converted = 0;
    for (variant, data) in rows {
        let entry: ProviderEntry =
            serde_json::from_str(&data).map_err(|_| row_error("provider_config", &variant))?;
        let (protected, changed) = protect_provider_entry(cipher, &entry, ProtectMode::Migration)?;
        if changed {
            conn.execute(
                "UPDATE provider_config SET data = ?1 WHERE variant = ?2",
                rusqlite::params![serde_json::to_string(&protected)?, variant],
            )?;
            converted += 1;
        }
    }
    Ok(converted)
}

fn convert_custom_provider(conn: &Connection, cipher: &SecretCipher) -> Result<usize> {
    let rows: Vec<(String, String)> = {
        let mut stmt = conn.prepare("SELECT id, data FROM custom_provider ORDER BY id")?;
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut converted = 0;
    for (id, data) in rows {
        let provider: CustomProvider =
            serde_json::from_str(&data).map_err(|_| row_error("custom_provider", &id))?;
        let (protected, changed) =
            protect_custom_provider(cipher, &provider, ProtectMode::Migration)?;
        if changed {
            conn.execute(
                "UPDATE custom_provider SET data = ?1 WHERE id = ?2",
                rusqlite::params![serde_json::to_string(&protected)?, id],
            )?;
            converted += 1;
        }
    }
    Ok(converted)
}

fn convert_integrations(
    conn: &Connection,
    cipher: &SecretCipher,
    paths: &dyn IntegrationSecretPaths,
) -> Result<usize> {
    let rows: Vec<(String, String, String)> = {
        let mut stmt = conn.prepare("SELECT id, kind, settings FROM integration ORDER BY id")?;
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut converted = 0;
    for (id, kind, settings) in rows {
        let settings: Value =
            serde_json::from_str(&settings).map_err(|_| row_error("integration", &id))?;
        let (protected, changed) = protect_integration_settings(
            cipher,
            paths,
            &id,
            &kind,
            &settings,
            ProtectMode::Migration,
        )?;
        if changed {
            conn.execute(
                "UPDATE integration SET settings = ?1 WHERE id = ?2",
                rusqlite::params![serde_json::to_string(&protected)?, id],
            )?;
            converted += 1;
        }
    }
    Ok(converted)
}

fn convert_agent_configs(conn: &Connection, cipher: &SecretCipher) -> Result<usize> {
    let rows: Vec<(String, String)> = {
        let mut stmt = conn.prepare("SELECT agent_id, data FROM agent_config ORDER BY agent_id")?;
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut converted = 0;
    for (agent_id, data) in rows {
        let config: AgentConfig =
            serde_json::from_str(&data).map_err(|_| row_error("agent_config", &agent_id))?;
        let (protected, _) =
            protect_agent_config(cipher, &agent_id, &config, ProtectMode::Migration)?;
        // Re-serialization also drops the pre-016 moved keys (`discord_token`, `telegram_token`,
        // `tools.discord`, `tools.telegram`, `tools.brave_search`, `tools.mcp_servers`), so their
        // plaintext does not survive in a row nobody re-saves. `core` is a real field and stays.
        let serialized = serde_json::to_string(&protected)?;
        if serialized != data {
            conn.execute(
                "UPDATE agent_config SET data = ?1 WHERE agent_id = ?2",
                rusqlite::params![serialized, agent_id],
            )?;
            converted += 1;
        }
    }
    Ok(converted)
}

fn convert_global_config(conn: &Connection, cipher: &SecretCipher) -> Result<usize> {
    let rows: Vec<(String, String)> = {
        let mut stmt = conn.prepare("SELECT key, data FROM global_config ORDER BY key")?;
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut converted = 0;
    for (key, data) in rows {
        let entry: GlobalConfigEntry =
            serde_json::from_str(&data).map_err(|_| row_error("global_config", &key))?;
        let (protected, changed) = protect_global_config(cipher, &entry, ProtectMode::Migration)?;
        if changed {
            conn.execute(
                "UPDATE global_config SET data = ?1 WHERE key = ?2",
                rusqlite::params![serde_json::to_string(&protected)?, key],
            )?;
            converted += 1;
        }
    }
    Ok(converted)
}

/// Remove the old plaintext from SQLite free pages and the WAL before the marker is written.
/// A failure here is a startup failure: the values are already protected, the marker is absent,
/// and the next start retries.
fn reclaim_plaintext(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(reclaim_error)?;
    conn.execute_batch("VACUUM;").map_err(reclaim_error)?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(reclaim_error)?;
    Ok(())
}

fn reclaim_error(error: rusqlite::Error) -> anyhow::Error {
    anyhow!(
        "encryption migration could not reclaim plaintext from the database (VACUUM/checkpoint failed): {error}"
    )
}

fn row_error(table: &str, record: &str) -> anyhow::Error {
    anyhow!(
        "could not read {table} row {record} during the encryption migration; fix or remove the row"
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::*;
    use crate::{
        encryption::{IntegrationSecretPaths, SecretCipher, cipher::key_id_for},
        integrations::IntegrationKinds,
        storage::{agent::AgentStorage, sqlite::SqliteStorage},
    };

    fn paths() -> Arc<dyn IntegrationSecretPaths> {
        Arc::new(IntegrationKinds::builtin())
    }

    fn cipher() -> SecretCipher {
        SecretCipher::from_key_for_test([0x31; 32])
    }

    fn harness() -> (SqliteStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = SqliteStorage::open_connection(dir.path().to_str().unwrap()).unwrap();
        let document_store: Arc<dyn crate::storage::document::DocumentStore> = Arc::new(
            crate::storage::document::LocalDocumentStore::new(dir.path().join("agents")),
        );
        (
            SqliteStorage::with_secrets(
                Arc::new(Mutex::new(conn)),
                document_store,
                cipher(),
                paths(),
            ),
            dir,
        )
    }

    fn seed_plaintext(storage: &SqliteStorage) {
        let conn = storage.conn.lock();
        conn.execute_batch(
            r#"
            INSERT INTO agent_secret (agent_id, name, value, created_at, updated_at, updated_by)
            VALUES ('dp-a', 'AGENT_TOKEN', 'agent-token-abc123', 'now', 'now', '"admin"');
            INSERT INTO agent_secret (agent_id, name, value, created_at, updated_at, updated_by)
            VALUES ('dp-b', 'AGENT_TOKEN', 'agent-token-abc123', 'now', 'now', '"admin"');

            INSERT INTO provider_config (variant, data) VALUES ('openai', json('{
                "id":"openai",
                "credentials":{"api_key":"provider-secret-abc123",
                               "endpoint":"https://endpoint.example",
                               "base_url":"https://base.example/v1",
                               "account_id":"acct-1"}}'));

            INSERT INTO custom_provider (id, data) VALUES ('acme', json('{
                "id":"acme","name":"Acme","protocol":"openai_compatible",
                "base_url":"https://acme.invalid/v1","api_key":"custom-secret-abc123",
                "headers":{"Authorization":"Bearer header-secret-abc123"},
                "models":[{"id":"acme-1"}]}'));

            INSERT INTO integration
                (id, kind, family, name, settings, enabled, scope, created_at, updated_at)
            VALUES ('discord-1','discord','channel','discord',
                    json('{"token":"discord-secret-abc123"}'), 1, 'owned', 'now', 'now');
            INSERT INTO integration
                (id, kind, family, name, settings, enabled, scope, created_at, updated_at)
            VALUES ('brave-1','brave_search','addon','brave',
                    json('{"api_key":"brave-secret-abc123","safesearch":true}'), 1, 'owned', 'now', 'now');
            INSERT INTO integration
                (id, kind, family, name, settings, enabled, scope, created_at, updated_at)
            VALUES ('mcp-1','mcp','mcp','crypto',
                    json('{"transport":"local","command":"cat","args":["--verbose"],
                           "env":{"GITHUB_TOKEN":"mcp-secret-abc123"}}'), 1, 'owned', 'now', 'now');
            INSERT INTO integration
                (id, kind, family, name, settings, enabled, scope, created_at, updated_at)
            VALUES ('legacy-1','retired_kind','mcp','legacy',
                    json('{"token":"legacy-secret-abc123"}'), 1, 'owned', 'now', 'now');

            INSERT INTO agent_config (agent_id, data) VALUES ('dp-env', json('{
                "name":"Dp Env","system_prompt":"be helpful","description":"test",
                "provider":"dummyplug","model":"dummyplug","thinking_depth":1,
                "tools":{"timeout":"30s","shell":{"environment":"local","path":"/bin/sh",
                         "env":{"SHELL_TOKEN":"shell-secret-abc123"}}},
                "silent_read_initiative_chance":0.0,"prompt_timeout":"30s",
                "heartbeat_interval":"60s","core":"core survives",
                "discord_token":"legacy-discord-token-abc123",
                "telegram_token":"legacy-telegram-token-abc123",
                "tools_extra_key":"legacy-tools-secret-abc123"}'));

            INSERT INTO global_config (key, data) VALUES ('default', json('{
                "key":"default","value":{"type":"Shell","data":{
                    "environment":"local","path":"/bin/sh",
                    "env":{"GLOBAL_TOKEN":"global-secret-abc123"}}}}'));
            "#,
        )
        .unwrap();
    }

    fn column(storage: &SqliteStorage, sql: &str) -> String {
        let conn = storage.conn.lock();
        conn.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    fn files_contain(dir: &std::path::Path, needle: &str) -> bool {
        let runtime = dir.join(".runtime");
        std::fs::read_dir(runtime)
            .map(|entries| {
                entries.flatten().any(|entry| {
                    let path = entry.path();
                    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                        return false;
                    };
                    if !name.starts_with("vizier.db") {
                        return false;
                    }
                    std::fs::read(&path)
                        .map(|bytes| {
                            let needle = needle.as_bytes();
                            bytes.windows(needle.len()).any(|w| w == needle)
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    }

    #[test]
    fn converts_all_six_tables_and_writes_the_marker() {
        let (storage, dir) = harness();
        seed_plaintext(&storage);
        {
            let conn = storage.conn.lock();
            let counts = migrate_secrets_to_encrypted(&conn, &cipher(), &*paths()).unwrap();
            assert_eq!(counts.agent_secret, 2);
            assert_eq!(counts.provider_config, 1);
            assert_eq!(counts.custom_provider, 1);
            assert_eq!(counts.integration, 4);
            assert_eq!(counts.agent_config, 1);
            assert_eq!(counts.global_config, 1);
            assert!(marker_present(&conn).unwrap());
        }

        assert!(
            column(
                &storage,
                "SELECT value FROM agent_secret WHERE agent_id='dp-a'"
            )
            .starts_with("enc:v1:")
        );
        assert!(
            column(
                &storage,
                "SELECT data FROM provider_config WHERE variant='openai'"
            )
            .contains("enc:v1:")
        );
        assert!(
            column(&storage, "SELECT data FROM custom_provider WHERE id='acme'")
                .contains("enc:v1:")
        );
        assert!(
            column(
                &storage,
                "SELECT settings FROM integration WHERE id='legacy-1'"
            )
            .contains("enc:v1:")
        );
        assert!(
            column(
                &storage,
                "SELECT data FROM agent_config WHERE agent_id='dp-env'"
            )
            .contains("enc:v1:")
        );
        assert!(
            column(
                &storage,
                "SELECT data FROM global_config WHERE key='default'"
            )
            .contains("enc:v1:")
        );

        // Siblings and the legacy scrub.
        let agent = column(
            &storage,
            "SELECT data FROM agent_config WHERE agent_id='dp-env'",
        );
        assert!(agent.contains("\"core\":\"core survives\""));
        assert!(agent.contains("\"model\":\"dummyplug\""));
        assert!(!agent.contains("legacy-discord-token"));
        assert!(!agent.contains("legacy-telegram-token"));
        assert!(
            column(
                &storage,
                "SELECT settings FROM integration WHERE id='brave-1'"
            )
            .contains("\"safesearch\":true")
        );

        for needle in [
            "agent-token-abc123",
            "provider-secret-abc123",
            "custom-secret-abc123",
            "header-secret-abc123",
            "discord-secret-abc123",
            "brave-secret-abc123",
            "mcp-secret-abc123",
            "shell-secret-abc123",
            "global-secret-abc123",
            "legacy-secret-abc123",
            "legacy-discord-token-abc123",
            "legacy-telegram-token-abc123",
            "legacy-tools-secret-abc123",
        ] {
            assert!(
                !files_contain(dir.path(), needle),
                "{needle} survived in a database file"
            );
        }
    }

    #[test]
    fn a_second_run_converts_nothing_and_changes_no_value() {
        let (storage, _dir) = harness();
        seed_plaintext(&storage);
        let conn = storage.conn.lock();
        migrate_secrets_to_encrypted(&conn, &cipher(), &*paths()).unwrap();
        let before: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT value FROM agent_secret ORDER BY agent_id")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap()
        };
        let counts = migrate_secrets_to_encrypted(&conn, &cipher(), &*paths()).unwrap();
        assert_eq!(counts, ConversionCount::default());
        let after: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT value FROM agent_secret ORDER BY agent_id")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap()
        };
        assert_eq!(before, after);
    }

    #[test]
    fn an_interrupted_run_resumes_without_reencrypting() {
        let (storage, dir) = harness();
        seed_plaintext(&storage);
        {
            let conn = storage.conn.lock();
            migrate_secrets_to_encrypted(&conn, &cipher(), &*paths()).unwrap();
            // Simulate a crash before the marker: the data is already protected.
            conn.execute(
                "DELETE FROM state WHERE key = ?1",
                rusqlite::params![MIGRATION_MARKER],
            )
            .unwrap();
        }
        let counts = {
            let conn = storage.conn.lock();
            migrate_secrets_to_encrypted(&conn, &cipher(), &*paths()).unwrap()
        };
        // The agent_config row may legitimately re-serialize to the same bytes, so it is not
        // counted; nothing else changes.
        assert_eq!(counts.agent_secret, 0);
        assert_eq!(counts.provider_config, 0);
        assert_eq!(counts.custom_provider, 0);
        assert_eq!(counts.integration, 0);
        assert_eq!(counts.global_config, 0);
        assert!(!files_contain(dir.path(), "agent-token-abc123"));
    }

    #[test]
    fn a_wrong_key_aborts_before_writing() {
        let (storage, _dir) = harness();
        seed_plaintext(&storage);
        let before = column(
            &storage,
            "SELECT value FROM agent_secret WHERE agent_id='dp-a'",
        );
        let original = cipher();
        // Protect one value first, so the second cipher sees an envelope it cannot open.
        {
            let conn = storage.conn.lock();
            let aad = crate::encryption::fields::aad("agent_secret", "dp-a", "AGENT_TOKEN");
            let envelope = original.encrypt("agent-token-abc123", &aad).unwrap();
            conn.execute(
                "UPDATE agent_secret SET value = ?1 WHERE agent_id='dp-a'",
                rusqlite::params![envelope],
            )
            .unwrap();
            let err = migrate_secrets_to_encrypted(
                &conn,
                &SecretCipher::from_key_for_test([0x32; 32]),
                &*paths(),
            )
            .unwrap_err();
            assert!(
                format!("{err}").contains("different encryption key")
                    || format!("{err}").contains("integrity")
            );
            // The transaction rolled back: the second agent is still plaintext.
            assert_eq!(
                conn.query_row(
                    "SELECT value FROM agent_secret WHERE agent_id='dp-b'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
                "agent-token-abc123"
            );
        }
        let _ = before;
    }

    #[test]
    fn malformed_rows_abort_with_record_identity_and_no_value() {
        let (storage, _dir) = harness();
        {
            let conn = storage.conn.lock();
            conn.execute(
                "INSERT INTO provider_config (variant, data) VALUES ('broken', '{not json')",
                [],
            )
            .unwrap();
        }
        let err = {
            let conn = storage.conn.lock();
            migrate_secrets_to_encrypted(&conn, &cipher(), &*paths()).unwrap_err()
        };
        let message = format!("{err}");
        assert!(message.contains("provider_config"), "{message}");
        assert!(message.contains("broken"), "{message}");
        assert!(!message.contains("not json"), "{message}");
    }

    #[test]
    fn an_unknown_integration_kind_decrypts_again() {
        use crate::storage::integration::IntegrationStorage;

        let (storage, _dir) = harness();
        seed_plaintext(&storage);
        {
            let conn = storage.conn.lock();
            migrate_secrets_to_encrypted(&conn, &cipher(), &*paths()).unwrap();
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let integration = rt
            .block_on(storage.get_integration("legacy-1"))
            .unwrap()
            .unwrap();
        assert_eq!(integration.settings["token"], "legacy-secret-abc123");
    }

    #[test]
    fn fts_and_vector_tables_survive_the_vacuum() {
        let (storage, _dir) = harness();
        {
            let conn = storage.conn.lock();
            conn.execute_batch(
                "INSERT INTO memory (id, agent_id, slug, visibility, data)
                 VALUES ('m1', 'dp-a', 'm1', 'private',
                         json('{\"title\":\"t\",\"content\":\"hello\",\"tags\":[]}'));
                 CREATE VIRTUAL TABLE IF NOT EXISTS document_index USING vec0(
                     embedding float[4]
                 );
                 INSERT INTO document_index(rowid, embedding) VALUES (1, '[1,2,3,4]');",
            )
            .unwrap();
        }
        {
            let conn = storage.conn.lock();
            migrate_secrets_to_encrypted(&conn, &cipher(), &*paths()).unwrap();
        }
        let conn = storage.conn.lock();
        let hits: i64 = conn
            .query_row(
                "SELECT count(*) FROM memory_fts WHERE memory_fts MATCH 'hello'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1);
        let vectors: i64 = conn
            .query_row("SELECT count(*) FROM document_index", [], |r| r.get(0))
            .unwrap();
        assert_eq!(vectors, 1);
    }

    #[test]
    fn startup_sees_core_after_the_conversion() {
        let (storage, _dir) = harness();
        seed_plaintext(&storage);
        {
            let conn = storage.conn.lock();
            migrate_secrets_to_encrypted(&conn, &cipher(), &*paths()).unwrap();
        }
        // This mirrors `migrate_agent_cores`: the legacy `core` field is still on the row, with
        // the shell env decrypted by the storage read path.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let config = rt.block_on(storage.get_agent("dp-env")).unwrap().unwrap();
        assert_eq!(config.core.as_deref(), Some("core survives"));
        assert_eq!(key_id_for(&[0x31; 32]).len(), 16);
    }
}
