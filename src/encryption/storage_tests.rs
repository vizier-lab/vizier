//! File-backed storage harness and the cross-surface assertions for
//! `specs/031-encrypt-agent-secrets` (tasks T008, T014).
//!
//! A file-backed database matters: the no-plaintext claim is about `vizier.db` and its WAL, which
//! an in-memory connection cannot exercise.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use parking_lot::Mutex;
use rusqlite::{Connection, ToSql};
use serde_json::{Value, json};

use crate::{
    config::shell::ShellConfig,
    encryption::{
        IntegrationSecretPaths, ProtectMode, SecretCipher,
        fields::{
            decrypt_agent_config, decrypt_agent_secret, decrypt_custom_provider,
            decrypt_global_config, decrypt_integration_settings, decrypt_provider_entry,
            protect_agent_config, protect_agent_secret, protect_custom_provider,
            protect_global_config, protect_integration_settings, protect_provider_entry,
        },
    },
    integrations::IntegrationKinds,
    schema::{
        AgentConfig, CustomProvider, EventAction, GlobalConfigEntry, Integration,
        IntegrationFamily, ProviderEntry, Requester,
    },
    storage::{
        VizierStorage,
        agent::AgentStorage,
        global_config::GlobalConfigStorage,
        integration::IntegrationStorage,
        provider::{CustomProviderStorage, ProviderStorage},
        secret::SecretStorage,
        sqlite::SqliteStorage,
    },
};
use chrono::Utc;

pub(crate) struct Harness {
    pub storage: SqliteStorage,
    pub dir: tempfile::TempDir,
}

pub(crate) fn fixed_cipher() -> SecretCipher {
    SecretCipher::from_key_for_test([0x42; 32])
}

pub(crate) fn file_backed() -> Harness {
    file_backed_with(fixed_cipher(), Arc::new(IntegrationKinds::builtin()))
}

pub(crate) fn file_backed_with(
    cipher: SecretCipher,
    secret_paths: Arc<dyn IntegrationSecretPaths>,
) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let conn = SqliteStorage::open_connection(dir.path().to_str().unwrap()).unwrap();
    let document_store: Arc<dyn crate::storage::document::DocumentStore> = Arc::new(
        crate::storage::document::LocalDocumentStore::new(dir.path().join("agents")),
    );
    let storage = SqliteStorage::with_secrets(
        Arc::new(Mutex::new(conn)),
        document_store,
        cipher,
        secret_paths,
    );
    Harness { storage, dir }
}

pub(crate) fn vizier_storage(harness: &Harness) -> VizierStorage {
    VizierStorage::new(harness.storage.clone())
}

pub(crate) fn raw_value(storage: &SqliteStorage, sql: &str, params: &[&dyn ToSql]) -> String {
    let conn = storage.conn.lock();
    conn.query_row(sql, params, |row| row.get(0)).unwrap()
}

pub(crate) fn raw_rows(storage: &SqliteStorage, sql: &str) -> Vec<String> {
    let conn = storage.conn.lock();
    let mut stmt = conn.prepare(sql).unwrap();
    stmt.query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

/// Flush the WAL so the bytes the assertions read are the bytes SQLite would leave behind.
pub(crate) fn flush(storage: &SqliteStorage) {
    let conn = storage.conn.lock();
    conn.execute_batch("PRAGMA wal_checkpoint(FULL);").unwrap();
}

pub(crate) fn data_files(dir: &Path) -> Vec<PathBuf> {
    let runtime = dir.join(".runtime");
    std::fs::read_dir(&runtime)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("vizier.db"))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Whether any byte of any database/WAL file contains `needle`.
pub(crate) fn files_contain(dir: &Path, needle: &str) -> bool {
    let needle = needle.as_bytes();
    data_files(dir).iter().any(|path| {
        std::fs::read(path)
            .map(|bytes| bytes.windows(needle.len()).any(|window| window == needle))
            .unwrap_or(false)
    })
}

pub(crate) fn assert_files_do_not_contain(dir: &Path, needles: &[&str]) {
    for needle in needles {
        assert!(
            !files_contain(dir, needle),
            "plaintext {needle:?} remains in a database file under {}",
            dir.display()
        );
    }
}

pub(crate) fn agent_config(env: Value) -> AgentConfig {
    serde_json::from_value(json!({
        "name": "Dp Env",
        "system_prompt": "be helpful",
        "description": "encryption storage test",
        "provider": "dummyplug",
        "model": "dummyplug",
        "thinking_depth": 1,
        "tools": {
            "timeout": "30s",
            "shell": {
                "environment": "local",
                "path": "/bin/sh",
                "env": env,
            },
        },
        "silent_read_initiative_chance": 0.0,
        "prompt_timeout": "30s",
        "heartbeat_interval": "60s",
        "core": "core survives",
    }))
    .unwrap()
}

pub(crate) fn provider(api_key: &str) -> ProviderEntry {
    serde_json::from_value(json!({
        "id": "openai",
        "credentials": {
            "api_key": api_key,
            "endpoint": "https://endpoint.example",
            "base_url": "https://base.example/v1",
            "account_id": "acct-1",
        },
    }))
    .unwrap()
}

pub(crate) fn custom_provider() -> CustomProvider {
    serde_json::from_value(json!({
        "id": "acme",
        "name": "Acme",
        "protocol": "openai_compatible",
        "base_url": "https://acme.invalid/v1",
        "api_key": "custom-secret-abc123",
        "headers": {
            "Authorization": "Bearer custom-header-secret-abc123",
            "X-Tenant": "tenant-secret-abc123",
        },
        "models": [{"id": "acme-1"}],
    }))
    .unwrap()
}

pub(crate) fn global_config() -> GlobalConfigEntry {
    serde_json::from_value(json!({
        "key": "default",
        "value": {
            "type": "Shell",
            "data": {
                "environment": "local",
                "path": "/bin/sh",
                "env": {"GLOBAL_TOKEN": "global-secret-abc123"},
            },
        },
    }))
    .unwrap()
}

pub(crate) fn integration(
    id: &str,
    kind: &str,
    family: IntegrationFamily,
    settings: Value,
) -> Integration {
    Integration {
        id: id.into(),
        kind: kind.into(),
        family,
        name: id.into(),
        settings,
        enabled: true,
        creator: Some(Requester::User("admin".into())),
        agents: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn created() -> crate::schema::EventDraft {
    crate::schema::EventDraft::new(Requester::User("admin".into()), EventAction::Created)
}

/// The test's only access to a shell's env map (the production API has no read helper because
/// nothing else needs one).
trait ShellEnv {
    fn env_value(&self, name: &str) -> Option<&String>;
}

impl ShellEnv for ShellConfig {
    fn env_value(&self, name: &str) -> Option<&String> {
        match self {
            Self::Docker(config) => config.env.as_ref()?.get(name),
            Self::Local(config) => config.env.as_ref()?.get(name),
        }
    }
}

#[tokio::test]
async fn every_surface_protects_its_secrets_and_keeps_its_siblings() {
    let h = file_backed();
    let s = &h.storage;
    let v = vizier_storage(&h);

    v.set_agent_secret(
        "dp-a",
        "AGENT_TOKEN",
        "agent-token-abc123",
        &Requester::User("admin".into()),
    )
    .await
    .unwrap();
    v.upsert_provider(&provider("provider-secret-abc123"))
        .await
        .unwrap();
    v.upsert_custom_provider(&custom_provider()).await.unwrap();
    v.upsert_global_config(&global_config()).await.unwrap();

    v.save_integration(
        &integration(
            "discord-1",
            "discord",
            IntegrationFamily::Channel,
            json!({"token": "discord-secret-abc123"}),
        ),
        Some("dp-a"),
        &[created()],
    )
    .await
    .unwrap();
    v.save_integration(
        &integration(
            "brave-1",
            "brave_search",
            IntegrationFamily::Addon,
            json!({"api_key": "brave-secret-abc123", "safesearch": true}),
        ),
        Some("dp-a"),
        &[created()],
    )
    .await
    .unwrap();
    v.save_integration(
        &integration(
            "mcp-1",
            "mcp",
            IntegrationFamily::Mcp,
            json!({"transport": "local", "command": "cat", "args": ["--verbose"],
                   "env": {"GITHUB_TOKEN": "mcp-secret-abc123"}}),
        ),
        Some("dp-a"),
        &[created()],
    )
    .await
    .unwrap();
    v.save_integration(
        &integration(
            "legacy-1",
            "retired_kind",
            IntegrationFamily::Mcp,
            json!({"token": "legacy-secret-abc123"}),
        ),
        Some("dp-a"),
        &[created()],
    )
    .await
    .unwrap();
    v.create_agent(
        "dp-env",
        &agent_config(json!({"SHELL_TOKEN": "shell-secret-abc123"})),
    )
    .await
    .unwrap();

    // Raw columns hold envelopes.
    for sql in [
        "SELECT value FROM agent_secret WHERE agent_id = 'dp-a' AND name = 'AGENT_TOKEN'",
        "SELECT data FROM provider_config WHERE variant = 'openai'",
        "SELECT data FROM custom_provider WHERE id = 'acme'",
        "SELECT data FROM global_config WHERE key = 'default'",
        "SELECT settings FROM integration WHERE id = 'discord-1'",
        "SELECT settings FROM integration WHERE id = 'brave-1'",
        "SELECT settings FROM integration WHERE id = 'mcp-1'",
        "SELECT settings FROM integration WHERE id = 'legacy-1'",
        "SELECT data FROM agent_config WHERE agent_id = 'dp-env'",
    ] {
        let raw = raw_value(s, sql, &[]);
        assert!(raw.contains("enc:v1:"), "not protected: {sql} -> {raw}");
    }

    // Reads return the exact plaintext.
    assert_eq!(
        v.get_agent_secret("dp-a", "AGENT_TOKEN")
            .await
            .unwrap()
            .as_deref(),
        Some("agent-token-abc123")
    );
    let p = v
        .get_provider(&"openai".parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        p.credentials.api_key.as_deref(),
        Some("provider-secret-abc123")
    );
    assert_eq!(
        p.credentials.endpoint.as_deref(),
        Some("https://endpoint.example")
    );
    let c = v
        .get_custom_provider(&"acme".parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(c.api_key.as_deref(), Some("custom-secret-abc123"));
    assert_eq!(
        c.headers["Authorization"],
        "Bearer custom-header-secret-abc123"
    );
    assert_eq!(
        v.get_global_config("default").await.unwrap().unwrap().key,
        "default"
    );
    let brave = v.get_integration("brave-1").await.unwrap().unwrap();
    assert_eq!(brave.settings["api_key"], "brave-secret-abc123");
    assert_eq!(brave.settings["safesearch"], json!(true));
    // Masking/redaction still runs on the decrypted value, exactly as before.
    let kinds = IntegrationKinds::builtin();
    let redacted = kinds.get("brave_search").unwrap().redact(&brave.settings);
    assert_eq!(redacted["api_key"], "••••c123");
    assert_eq!(redacted["safesearch"], json!(true));
    let mcp = v.get_integration("mcp-1").await.unwrap().unwrap();
    assert_eq!(mcp.settings["env"]["GITHUB_TOKEN"], "mcp-secret-abc123");
    assert_eq!(mcp.settings["command"], "cat");
    let agent = v.get_agent("dp-env").await.unwrap().unwrap();
    assert_eq!(
        agent
            .tools
            .shell
            .unwrap()
            .env_value("SHELL_TOKEN")
            .map(String::as_str),
        Some("shell-secret-abc123")
    );

    // Listing masks after decryption.
    let listed = v.list_agent_secrets("dp-a").await.unwrap();
    assert_eq!(listed[0].masked, "••••c123");

    // No plaintext in the database or WAL.
    flush(s);
    assert_files_do_not_contain(
        h.dir.path(),
        &[
            "agent-token-abc123",
            "provider-secret-abc123",
            "custom-secret-abc123",
            "custom-header-secret-abc123",
            "tenant-secret-abc123",
            "discord-secret-abc123",
            "brave-secret-abc123",
            "mcp-secret-abc123",
            "shell-secret-abc123",
            "legacy-secret-abc123",
            "global-secret-abc123",
        ],
    );

    // Siblings remain readable bytes.
    assert!(files_contain(h.dir.path(), "\"safesearch\":true"));
    assert!(files_contain(h.dir.path(), "https://acme.invalid/v1"));
    assert!(files_contain(h.dir.path(), "\"model\":\"dummyplug\""));
}

/// A tampered protected value must fail the read, not return a short list or a partial value.
#[tokio::test]
async fn tampered_values_fail_instead_of_disappearing() {
    let h = file_backed();
    let s = &h.storage;
    let v = vizier_storage(&h);
    v.set_agent_secret("dp-a", "TOKEN", "secret", &Requester::User("admin".into()))
        .await
        .unwrap();
    {
        let conn = s.conn.lock();
        conn.execute("UPDATE agent_secret SET value = value || 'x'", [])
            .unwrap();
    }
    assert!(v.get_agent_secret("dp-a", "TOKEN").await.is_err());
    assert!(v.list_agent_secrets("dp-a").await.is_err());
}

/// The shell-env exception is deliberate: protected at rest, still returned by the authorized
/// agent configuration read (T034).
#[tokio::test]
async fn shell_env_is_protected_at_rest_but_visible_in_the_agent_config() {
    let h = file_backed();
    let v = vizier_storage(&h);
    v.create_agent(
        "dp-env",
        &agent_config(json!({"SHELL_TOKEN": "shell-secret-abc123"})),
    )
    .await
    .unwrap();
    flush(&h.storage);
    assert!(!files_contain(h.dir.path(), "shell-secret-abc123"));
    let config = v.get_agent("dp-env").await.unwrap().unwrap();
    assert_eq!(
        config
            .tools
            .shell
            .unwrap()
            .env_value("SHELL_TOKEN")
            .map(String::as_str),
        Some("shell-secret-abc123")
    );
}

/// `ProtectMode::Migration` is idempotent on the same value, and the normal write path always
/// produces a different envelope (no accidental double-encryption guard needed at write time).
#[test]
fn write_mode_and_migration_mode_agree_on_plaintext() {
    let cipher = fixed_cipher();
    let (stored, changed) =
        protect_agent_secret(&cipher, "dp-a", "T", "value", ProtectMode::Write).unwrap();
    assert!(changed);
    let (again, changed) =
        protect_agent_secret(&cipher, "dp-a", "T", &stored, ProtectMode::Migration).unwrap();
    assert!(!changed);
    assert_eq!(again, stored);
}

// Keep the helper imports that only the public helpers use, and pin the round-trip helpers used
// by per-surface tests: a compile error here means an AAD field path changed.
#[allow(dead_code)]
fn helpers_round_trip(cipher: &SecretCipher, paths: &dyn IntegrationSecretPaths) {
    let p = provider("k");
    let _ = decrypt_provider_entry(
        cipher,
        &protect_provider_entry(cipher, &p, ProtectMode::Write)
            .unwrap()
            .0,
    );
    let c = custom_provider();
    let _ = decrypt_custom_provider(
        cipher,
        &protect_custom_provider(cipher, &c, ProtectMode::Write)
            .unwrap()
            .0,
    );
    let a = agent_config(json!({"A": "b"}));
    let _ = decrypt_agent_config(
        cipher,
        "a",
        &protect_agent_config(cipher, "a", &a, ProtectMode::Write)
            .unwrap()
            .0,
    );
    let g = global_config();
    let _ = decrypt_global_config(
        cipher,
        &protect_global_config(cipher, &g, ProtectMode::Write)
            .unwrap()
            .0,
    );
    let i = json!({"token": "t"});
    let _ = decrypt_integration_settings(
        cipher,
        paths,
        "i",
        "discord",
        &protect_integration_settings(cipher, paths, "i", "discord", &i, ProtectMode::Write)
            .unwrap()
            .0,
    );
    let _ = decrypt_agent_secret(
        cipher,
        "a",
        "n",
        &protect_agent_secret(cipher, "a", "n", "v", ProtectMode::Write)
            .unwrap()
            .0,
    );
}
