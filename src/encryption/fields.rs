//! Field-level protection for every secret-bearing surface
//! (`specs/031-encrypt-agent-secrets/data-model.md` §2, §4).
//!
//! Every helper both encrypts and decrypts its shape, and both storage and the one-time
//! migration call these helpers, so the AAD layout has exactly one implementation. Non-secret
//! siblings are never touched.

use std::collections::HashMap;

use anyhow::{Result, bail};
use serde_json::Value;

use crate::{
    config::shell::ShellConfig,
    encryption::{
        IntegrationSecretPaths,
        cipher::{CipherFailure, SecretCipher},
    },
    schema::{AgentConfig, CustomProvider, GlobalConfigEntry, GlobalConfigValue, ProviderEntry},
};

const AAD_PREFIX: &str = "vizier:secrets:v1";

/// Whether a write should always encrypt, or whether the one-time migration should validate and
/// skip a value that is already protected. Normal storage never uses `Migration`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectMode {
    Write,
    Migration,
}

/// The full associated data for one value: prefix, table, record identity and field path.
pub fn aad(table: &str, record: &str, field: &str) -> String {
    format!("{AAD_PREFIX}\x1f{table}\x1f{record}\x1f{field}")
}

fn value_error(table: &str, record: &str, field: &str, failure: CipherFailure) -> anyhow::Error {
    if failure == CipherFailure::UnknownVersion {
        anyhow::anyhow!(
            "encrypted value in {table} ({record}, {field}) uses an unknown format version"
        )
    } else {
        anyhow::anyhow!("encrypted value in {table} ({record}, {field}) failed its integrity check")
    }
}

/// Protect one string. Returns the stored form and whether it changed.
fn protect_value(
    cipher: &SecretCipher,
    table: &str,
    record: &str,
    field: &str,
    plaintext: &str,
    mode: ProtectMode,
) -> Result<(String, bool)> {
    let aad = aad(table, record, field);
    if mode == ProtectMode::Migration
        && cipher
            .open(plaintext, &aad)
            .map_err(|e| value_error(table, record, field, e))?
            .is_some()
    {
        return Ok((plaintext.to_string(), false));
    }
    Ok((cipher.encrypt(plaintext, &aad)?, true))
}

fn decrypt_value(
    cipher: &SecretCipher,
    table: &str,
    record: &str,
    field: &str,
    stored: &str,
) -> Result<String> {
    let aad = aad(table, record, field);
    cipher
        .decrypt(stored, &aad)
        .map_err(|e| value_error(table, record, field, e))
}

// ---------------------------------------------------------------- agent secrets

pub fn protect_agent_secret(
    cipher: &SecretCipher,
    agent_id: &str,
    name: &str,
    value: &str,
    mode: ProtectMode,
) -> Result<(String, bool)> {
    protect_value(cipher, "agent_secret", agent_id, name, value, mode)
}

pub fn decrypt_agent_secret(
    cipher: &SecretCipher,
    agent_id: &str,
    name: &str,
    stored: &str,
) -> Result<String> {
    decrypt_value(cipher, "agent_secret", agent_id, name, stored)
}

// ---------------------------------------------------------------- providers

pub fn protect_provider_entry(
    cipher: &SecretCipher,
    entry: &ProviderEntry,
    mode: ProtectMode,
) -> Result<(ProviderEntry, bool)> {
    let mut out = entry.clone();
    let record = entry.id.as_str();
    let mut changed = false;

    if let Some(value) = &out.credentials.api_key {
        let (stored, did) = protect_value(
            cipher,
            "provider_config",
            record,
            "credentials.api_key",
            value,
            mode,
        )?;
        out.credentials.api_key = Some(stored);
        changed |= did;
    }
    if let Some(value) = &out.credentials.access_token {
        let (stored, did) = protect_value(
            cipher,
            "provider_config",
            record,
            "credentials.access_token",
            value,
            mode,
        )?;
        out.credentials.access_token = Some(stored);
        changed |= did;
    }
    Ok((out, changed))
}

pub fn decrypt_provider_entry(
    cipher: &SecretCipher,
    entry: &ProviderEntry,
) -> Result<ProviderEntry> {
    let mut out = entry.clone();
    let record = entry.id.as_str();
    if let Some(value) = &out.credentials.api_key {
        out.credentials.api_key = Some(decrypt_value(
            cipher,
            "provider_config",
            record,
            "credentials.api_key",
            value,
        )?);
    }
    if let Some(value) = &out.credentials.access_token {
        out.credentials.access_token = Some(decrypt_value(
            cipher,
            "provider_config",
            record,
            "credentials.access_token",
            value,
        )?);
    }
    Ok(out)
}

// ---------------------------------------------------------------- custom providers

pub fn protect_custom_provider(
    cipher: &SecretCipher,
    provider: &CustomProvider,
    mode: ProtectMode,
) -> Result<(CustomProvider, bool)> {
    let mut out = provider.clone();
    let record = provider.id.as_str();
    let mut changed = false;

    if let Some(value) = &out.api_key {
        let (stored, did) =
            protect_value(cipher, "custom_provider", record, "api_key", value, mode)?;
        out.api_key = Some(stored);
        changed |= did;
    }
    for (name, value) in out.headers.iter_mut() {
        let field = format!("headers.{name}");
        let (stored, did) = protect_value(cipher, "custom_provider", record, &field, value, mode)?;
        *value = stored;
        changed |= did;
    }
    Ok((out, changed))
}

pub fn decrypt_custom_provider(
    cipher: &SecretCipher,
    provider: &CustomProvider,
) -> Result<CustomProvider> {
    let mut out = provider.clone();
    let record = provider.id.as_str();
    if let Some(value) = &out.api_key {
        out.api_key = Some(decrypt_value(
            cipher,
            "custom_provider",
            record,
            "api_key",
            value,
        )?);
    }
    for (name, value) in out.headers.iter_mut() {
        let field = format!("headers.{name}");
        *value = decrypt_value(cipher, "custom_provider", record, &field, value)?;
    }
    Ok(out)
}

// ---------------------------------------------------------------- shell environment

fn shell_env_mut(shell: &mut ShellConfig) -> Option<&mut HashMap<String, String>> {
    match shell {
        ShellConfig::Docker(config) => config.env.as_mut(),
        ShellConfig::Local(config) => config.env.as_mut(),
    }
}

fn protect_shell_env(
    cipher: &SecretCipher,
    table: &str,
    record: &str,
    prefix: &str,
    shell: &mut ShellConfig,
    mode: ProtectMode,
) -> Result<bool> {
    let Some(env) = shell_env_mut(shell) else {
        return Ok(false);
    };
    let mut changed = false;
    for (name, value) in env.iter_mut() {
        let field = format!("{prefix}{name}");
        let (stored, did) = protect_value(cipher, table, record, &field, value, mode)?;
        *value = stored;
        changed |= did;
    }
    Ok(changed)
}

fn decrypt_shell_env(
    cipher: &SecretCipher,
    table: &str,
    record: &str,
    prefix: &str,
    shell: &mut ShellConfig,
) -> Result<()> {
    let Some(env) = shell_env_mut(shell) else {
        return Ok(());
    };
    for (name, value) in env.iter_mut() {
        let field = format!("{prefix}{name}");
        *value = decrypt_value(cipher, table, record, &field, value)?;
    }
    Ok(())
}

pub fn protect_agent_config(
    cipher: &SecretCipher,
    agent_id: &str,
    config: &AgentConfig,
    mode: ProtectMode,
) -> Result<(AgentConfig, bool)> {
    let mut out = config.clone();
    let changed = match out.tools.shell.as_mut() {
        Some(shell) => protect_shell_env(
            cipher,
            "agent_config",
            agent_id,
            "tools.shell.env.",
            shell,
            mode,
        )?,
        None => false,
    };
    Ok((out, changed))
}

pub fn decrypt_agent_config(
    cipher: &SecretCipher,
    agent_id: &str,
    config: &AgentConfig,
) -> Result<AgentConfig> {
    let mut out = config.clone();
    if let Some(shell) = out.tools.shell.as_mut() {
        decrypt_shell_env(cipher, "agent_config", agent_id, "tools.shell.env.", shell)?;
    }
    Ok(out)
}

pub fn protect_global_config(
    cipher: &SecretCipher,
    entry: &GlobalConfigEntry,
    mode: ProtectMode,
) -> Result<(GlobalConfigEntry, bool)> {
    let mut out = entry.clone();
    let record = entry.key.as_str();
    let GlobalConfigValue::Shell(shell) = &mut out.value;
    let changed = protect_shell_env(
        cipher,
        "global_config",
        record,
        "value.data.env.",
        shell,
        mode,
    )?;
    Ok((out, changed))
}

pub fn decrypt_global_config(
    cipher: &SecretCipher,
    entry: &GlobalConfigEntry,
) -> Result<GlobalConfigEntry> {
    let mut out = entry.clone();
    let record = entry.key.as_str();
    let GlobalConfigValue::Shell(shell) = &mut out.value;
    decrypt_shell_env(cipher, "global_config", record, "value.data.env.", shell)?;
    Ok(out)
}

// ---------------------------------------------------------------- integrations

/// Protect the kind-declared secret paths in an integration's settings.
///
/// A known kind gets field-level protection, so transport/command/args/URL/safe-search remain
/// readable. A kind this binary no longer knows gets the whole settings JSON in one envelope:
/// there is no field classification to trust. `state.agents` (dispatch) and the catalogue are
/// the only callers, so an unknown kind is a genuine legacy/removed kind.
pub fn protect_integration_settings(
    cipher: &SecretCipher,
    paths: &dyn IntegrationSecretPaths,
    id: &str,
    kind: &str,
    settings: &Value,
    mode: ProtectMode,
) -> Result<(Value, bool)> {
    let Some(secret_paths) = paths.secret_paths(kind) else {
        return protect_whole_settings(cipher, id, settings, mode);
    };

    // A kind that used to be unknown may already be whole-blob protected. Leave it that way:
    // its reader decrypts the blob first, then re-saves it field-level on the next write.
    if let Value::String(raw) = settings
        && SecretCipher::is_protected(raw)
    {
        let (stored, _) = protect_value(cipher, "integration", id, "settings", raw, mode)?;
        return Ok((Value::String(stored), false));
    }

    let Value::Object(map) = settings else {
        bail!("integration '{id}' settings must be an object");
    };
    let mut out = map.clone();
    let mut changed = false;

    for path in secret_paths {
        if let Some(base) = path.strip_suffix(".*") {
            match out.get_mut(base) {
                None => {}
                Some(Value::Object(values)) => {
                    for (name, value) in values.iter_mut() {
                        let Value::String(text) = value else {
                            bail!("integration '{id}' field '{base}.{name}' must be a string");
                        };
                        let field = format!("{base}.{name}");
                        let (stored, did) =
                            protect_value(cipher, "integration", id, &field, text, mode)?;
                        *text = stored;
                        changed |= did;
                    }
                }
                Some(_) => bail!("integration '{id}' field '{base}' must be an object"),
            }
        } else {
            match out.get_mut(&path) {
                None => {}
                Some(Value::String(text)) => {
                    let (stored, did) =
                        protect_value(cipher, "integration", id, &path, text, mode)?;
                    *text = stored;
                    changed |= did;
                }
                Some(_) => bail!("integration '{id}' field '{path}' must be a string"),
            }
        }
    }

    Ok((Value::Object(out), changed))
}

fn protect_whole_settings(
    cipher: &SecretCipher,
    id: &str,
    settings: &Value,
    mode: ProtectMode,
) -> Result<(Value, bool)> {
    let json = match settings {
        Value::String(text) => text.clone(),
        other => serde_json::to_string(other)?,
    };
    let (stored, changed) = protect_value(cipher, "integration", id, "settings", &json, mode)?;
    Ok((Value::String(stored), changed))
}

pub fn decrypt_integration_settings(
    cipher: &SecretCipher,
    paths: &dyn IntegrationSecretPaths,
    id: &str,
    kind: &str,
    settings: &Value,
) -> Result<Value> {
    let Some(secret_paths) = paths.secret_paths(kind) else {
        return decrypt_whole_settings(cipher, id, settings);
    };

    // Whole-blob fallback from a release that did not know this kind: decrypt first, then apply
    // the field-level paths to the recovered JSON.
    if let Value::String(raw) = settings
        && SecretCipher::is_protected(raw)
    {
        let json = decrypt_value(cipher, "integration", id, "settings", raw)?;
        let parsed: Value = serde_json::from_str(&json).unwrap_or(Value::String(json));
        return decrypt_integration_settings(cipher, paths, id, kind, &parsed);
    }

    let Value::Object(map) = settings else {
        bail!("integration '{id}' settings must be an object");
    };
    let mut out = map.clone();
    for path in secret_paths {
        if let Some(base) = path.strip_suffix(".*") {
            match out.get_mut(base) {
                None => {}
                Some(Value::Object(values)) => {
                    for (name, value) in values.iter_mut() {
                        let Value::String(text) = value else {
                            bail!("integration '{id}' field '{base}.{name}' must be a string");
                        };
                        let field = format!("{base}.{name}");
                        *text = decrypt_value(cipher, "integration", id, &field, text)?;
                    }
                }
                Some(_) => bail!("integration '{id}' field '{base}' must be an object"),
            }
        } else {
            match out.get_mut(&path) {
                None => {}
                Some(Value::String(text)) => {
                    *text = decrypt_value(cipher, "integration", id, &path, text)?;
                }
                Some(_) => bail!("integration '{id}' field '{path}' must be a string"),
            }
        }
    }
    Ok(Value::Object(out))
}

fn decrypt_whole_settings(cipher: &SecretCipher, id: &str, settings: &Value) -> Result<Value> {
    let Value::String(raw) = settings else {
        bail!("integration '{id}' settings were not protected");
    };
    let json = decrypt_value(cipher, "integration", id, "settings", raw)?;
    Ok(serde_json::from_str(&json).unwrap_or(Value::String(json)))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{
        config::shell::{DockerShellConfig, DockerSourceConfig, LocalShellConfig},
        encryption::cipher::SecretCipher,
        schema::{CustomModel, CustomProtocol, ProviderCredentials, agent::AgentToolsConfig},
    };

    fn cipher() -> SecretCipher {
        SecretCipher::from_key_for_test([11u8; 32])
    }

    struct Paths;

    impl IntegrationSecretPaths for Paths {
        fn secret_paths(&self, kind: &str) -> Option<Vec<String>> {
            match kind {
                "discord" | "telegram" => Some(vec!["token".into()]),
                "brave_search" => Some(vec!["api_key".into()]),
                "mcp" => Some(vec!["env.*".into()]),
                _ => None,
            }
        }
    }

    fn provider(api_key: &str, access_token: Option<&str>) -> ProviderEntry {
        ProviderEntry {
            id: "openai".parse().unwrap(),
            credentials: ProviderCredentials {
                api_key: Some(api_key.into()),
                endpoint: Some("https://endpoint.example".into()),
                base_url: Some("https://base.example/v1".into()),
                access_token: access_token.map(str::to_string),
                account_id: Some("acct".into()),
            },
        }
    }

    fn custom() -> CustomProvider {
        let mut headers = std::collections::BTreeMap::new();
        headers.insert("Authorization".into(), "Bearer header-secret".into());
        headers.insert("X-Tenant".into(), "tenant-secret".into());
        CustomProvider {
            id: "acme".parse().unwrap(),
            name: "Acme".into(),
            protocol: CustomProtocol::OpenaiCompatible,
            base_url: "https://acme.invalid/v1".into(),
            api_key: Some("custom-secret".into()),
            headers,
            models: vec![CustomModel {
                id: "acme-1".into(),
                ..Default::default()
            }],
            models_fetched_at: None,
        }
    }

    fn agent_config(shell: ShellConfig) -> AgentConfig {
        AgentConfig {
            name: "Dp A".into(),
            owner_id: None,
            shared_to: vec![],
            system_prompt: Some("be helpful".into()),
            description: Some("test".into()),
            provider: "dummyplug".parse().unwrap(),
            model: "dummyplug".into(),
            thinking_depth: 1,
            checkpoint_threshold: 0.8,
            tools: AgentToolsConfig {
                timeout: std::time::Duration::from_secs(30).into(),
                shell: Some(shell),
                fetch: Default::default(),
                http_client: Default::default(),
                tts: Default::default(),
                stt: Default::default(),
                read_image: Default::default(),
                image_gen: Default::default(),
            },
            silent_read_initiative_chance: 0.0,
            max_tokens: None,
            context_window: None,
            include_documents: None,
            prompt_timeout: std::time::Duration::from_secs(30).into(),
            documents: vec![],
            heartbeat_interval: std::time::Duration::from_secs(60).into(),
            core: Some("core text".into()),
            dream_enabled: false,
            dream_schedule: None,
            dream_provider: None,
            dream_model: None,
            avatar_url: None,
            embedding: None,
            indexer: None,
            chunking: Default::default(),
            auto_context: Default::default(),
            completion_retry: Default::default(),
        }
    }

    fn local(env: &[(&str, &str)]) -> ShellConfig {
        ShellConfig::Local(LocalShellConfig {
            path: "/bin/sh".into(),
            env: Some(
                env.iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ),
        })
    }

    #[test]
    fn agent_secret_round_trips_and_is_bound_to_its_record() {
        let c = cipher();
        let (stored, changed) =
            protect_agent_secret(&c, "dp-a", "TOKEN", "agent-token", ProtectMode::Write).unwrap();
        assert!(changed);
        assert!(stored.starts_with("enc:v1:"));
        assert_eq!(
            decrypt_agent_secret(&c, "dp-a", "TOKEN", &stored).unwrap(),
            "agent-token"
        );
        assert!(decrypt_agent_secret(&c, "dp-b", "TOKEN", &stored).is_err());
        assert!(decrypt_agent_secret(&c, "dp-a", "OTHER", &stored).is_err());
    }

    #[test]
    fn provider_secrets_are_protected_and_siblings_are_not() {
        let c = cipher();
        let (protected, changed) = protect_provider_entry(
            &c,
            &provider("provider-secret", Some("token-secret")),
            ProtectMode::Write,
        )
        .unwrap();
        assert!(changed);
        assert!(
            protected
                .credentials
                .api_key
                .as_ref()
                .unwrap()
                .starts_with("enc:v1:")
        );
        assert!(
            protected
                .credentials
                .access_token
                .as_ref()
                .unwrap()
                .starts_with("enc:v1:")
        );
        // Siblings stay readable.
        assert_eq!(
            protected.credentials.endpoint.as_deref(),
            Some("https://endpoint.example")
        );
        assert_eq!(
            protected.credentials.base_url.as_deref(),
            Some("https://base.example/v1")
        );
        assert_eq!(protected.credentials.account_id.as_deref(), Some("acct"));

        assert_eq!(
            decrypt_provider_entry(&c, &protected).unwrap(),
            provider("provider-secret", Some("token-secret"))
        );
    }

    #[test]
    fn custom_provider_protects_the_key_and_every_header_value_only() {
        let c = cipher();
        let original = custom();
        let (protected, changed) =
            protect_custom_provider(&c, &original, ProtectMode::Write).unwrap();
        assert!(changed);
        assert!(protected.api_key.as_ref().unwrap().starts_with("enc:v1:"));
        assert!(protected.headers["Authorization"].starts_with("enc:v1:"));
        assert!(protected.headers["X-Tenant"].starts_with("enc:v1:"));
        assert_eq!(protected.base_url, original.base_url);
        assert_eq!(protected.name, original.name);
        assert_eq!(protected.models, original.models);
        assert_eq!(
            protected.headers.keys().collect::<Vec<_>>(),
            original.headers.keys().collect::<Vec<_>>()
        );

        assert_eq!(decrypt_custom_provider(&c, &protected).unwrap(), original);
    }

    #[test]
    fn agent_shell_env_is_protected_for_local_and_docker_but_the_rest_is_not() {
        let c = cipher();
        for shell in [
            local(&[("SHELL_TOKEN", "shell-secret"), ("OTHER", "")]),
            ShellConfig::Docker(DockerShellConfig {
                image: DockerSourceConfig::Pull {
                    name: "ubuntu:latest".into(),
                },
                container_name: "vizier".into(),
                env: Some(HashMap::from([(
                    "SHELL_TOKEN".into(),
                    "shell-secret".into(),
                )])),
            }),
        ] {
            let config = agent_config(shell);
            let (protected, changed) =
                protect_agent_config(&c, "dp-env", &config, ProtectMode::Write).unwrap();
            assert!(changed);
            assert_eq!(protected.name, "Dp A");
            assert_eq!(protected.model, "dummyplug");
            assert_eq!(protected.core.as_deref(), Some("core text"));
            let raw = serde_json::to_string(&protected).unwrap();
            assert!(!raw.contains("shell-secret"));
            assert_eq!(
                decrypt_agent_config(&c, "dp-env", &protected).unwrap().name,
                "Dp A"
            );
        }
    }

    #[test]
    fn global_config_protects_shell_env_values() {
        let c = cipher();
        let entry = GlobalConfigEntry {
            key: "default".into(),
            value: GlobalConfigValue::Shell(local(&[("GLOBAL_TOKEN", "global-secret")])),
        };
        let (protected, changed) = protect_global_config(&c, &entry, ProtectMode::Write).unwrap();
        assert!(changed);
        let raw = serde_json::to_string(&protected).unwrap();
        assert!(raw.contains("enc:v1:"));
        assert!(!raw.contains("global-secret"));
        let back = decrypt_global_config(&c, &protected).unwrap();
        assert_eq!(
            serde_json::to_string(&back).unwrap(),
            serde_json::to_string(&entry).unwrap()
        );
    }

    #[test]
    fn known_integration_kinds_protect_only_their_declared_paths() {
        let c = cipher();
        let paths = Paths;
        let brave = json!({"api_key": "brave-secret", "safesearch": true});
        let (protected, changed) = protect_integration_settings(
            &c,
            &paths,
            "b1",
            "brave_search",
            &brave,
            ProtectMode::Write,
        )
        .unwrap();
        assert!(changed);
        assert!(
            protected["api_key"]
                .as_str()
                .unwrap()
                .starts_with("enc:v1:")
        );
        assert_eq!(protected["safesearch"], json!(true));
        assert_eq!(
            decrypt_integration_settings(&c, &paths, "b1", "brave_search", &protected).unwrap(),
            brave
        );

        let mcp = json!({"transport": "local", "command": "cat", "args": ["--verbose"],
                         "env": {"GITHUB_TOKEN": "mcp-secret"}});
        let (protected, changed) =
            protect_integration_settings(&c, &paths, "m1", "mcp", &mcp, ProtectMode::Write)
                .unwrap();
        assert!(changed);
        assert!(
            protected["env"]["GITHUB_TOKEN"]
                .as_str()
                .unwrap()
                .starts_with("enc:v1:")
        );
        assert_eq!(protected["transport"], json!("local"));
        assert_eq!(protected["command"], json!("cat"));
        assert_eq!(protected["args"], json!(["--verbose"]));
        assert_eq!(
            decrypt_integration_settings(&c, &paths, "m1", "mcp", &protected).unwrap(),
            mcp
        );
    }

    #[test]
    fn an_unknown_integration_kind_is_one_whole_settings_envelope() {
        let c = cipher();
        let paths = Paths;
        let settings = json!({"token": "legacy-secret", "other": "visible?"});
        let (protected, changed) = protect_integration_settings(
            &c,
            &paths,
            "x1",
            "retired_kind",
            &settings,
            ProtectMode::Write,
        )
        .unwrap();
        assert!(changed);
        assert!(protected.is_string());
        assert!(
            !serde_json::to_string(&protected)
                .unwrap()
                .contains("legacy-secret")
        );
        assert_eq!(
            decrypt_integration_settings(&c, &paths, "x1", "retired_kind", &protected).unwrap(),
            settings
        );
    }

    #[test]
    fn migration_mode_skips_valid_envelopes_and_encrypts_plaintext() {
        let c = cipher();
        let paths = Paths;
        let first = json!({"api_key": "brave-secret", "safesearch": true});
        let (protected, changed) = protect_integration_settings(
            &c,
            &paths,
            "b1",
            "brave_search",
            &first,
            ProtectMode::Migration,
        )
        .unwrap();
        assert!(changed);
        let before = protected.clone();
        let (again, changed) = protect_integration_settings(
            &c,
            &paths,
            "b1",
            "brave_search",
            &before,
            ProtectMode::Migration,
        )
        .unwrap();
        assert!(!changed);
        assert_eq!(again, before);

        let (secret, changed) =
            protect_agent_secret(&c, "dp-a", "TOKEN", "plain", ProtectMode::Migration).unwrap();
        assert!(changed);
        assert!(secret.starts_with("enc:v1:"));
    }

    #[test]
    fn non_string_secret_values_are_errors() {
        let c = cipher();
        let paths = Paths;
        let bad = json!({"token": 5});
        assert!(
            protect_integration_settings(&c, &paths, "t1", "telegram", &bad, ProtectMode::Write)
                .is_err()
        );
        let bad_env = json!({"env": {"TOKEN": 5}});
        assert!(
            protect_integration_settings(&c, &paths, "m1", "mcp", &bad_env, ProtectMode::Write)
                .is_err()
        );
        let not_object = json!(["token"]);
        assert!(
            protect_integration_settings(
                &c,
                &paths,
                "t1",
                "telegram",
                &not_object,
                ProtectMode::Write
            )
            .is_err()
        );
    }

    #[test]
    fn a_moved_envelope_fails_its_aad() {
        let c = cipher();
        let (stored, _) =
            protect_agent_secret(&c, "dp-a", "TOKEN", "agent-token", ProtectMode::Write).unwrap();
        assert!(decrypt_agent_secret(&c, "dp-a", "TOKEN", &stored).is_ok());
        assert!(decrypt_agent_secret(&c, "dp-a", "RENAMED", &stored).is_err());
    }

    #[test]
    fn empty_values_are_still_protected() {
        let c = cipher();
        let (stored, _) =
            protect_agent_secret(&c, "dp-a", "EMPTY", "", ProtectMode::Write).unwrap();
        assert!(stored.starts_with("enc:v1:"));
        assert_eq!(
            decrypt_agent_secret(&c, "dp-a", "EMPTY", &stored).unwrap(),
            ""
        );
    }
}
