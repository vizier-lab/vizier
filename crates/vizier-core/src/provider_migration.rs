//! `migrate_provider_ids_v1`: the one-time rewrite of pre-015 provider ids and credentials
//! (contract `provider-id-migration.md`). Idempotent: it runs once, guarded by a `state`
//! marker, and rows already in the new shape are left alone.
//!
//! - `provider_config` rows move from the tagged `ProviderEntryConfig` under their legacy name
//!   to `ProviderCredentials` under their `ProviderId`.
//! - A saved `base_url` decides the target: one equal to a catalogue provider's `api` maps to
//!   that provider (`moonshot` + `api.moonshot.cn` → `moonshotai-cn`); one that matches nothing
//!   and isn't the old default becomes a custom provider `<legacy>-custom`.
//! - The legacy `custom` row (or `CUSTOM_BASE_URL`/`CUSTOM_API_KEY`) becomes the custom
//!   provider `custom`.
//! - `galadriel` and `mira` credentials are deleted; agents on them are left to fail with
//!   "not available".
//! - `agent_config` rows get `provider`, `dream_provider` and `tools.read_image.settings.provider`
//!   rewritten with this deployment's map.

use std::collections::{BTreeMap, HashMap};

use rusqlite::OptionalExtension;
use serde_json::Value;

use crate::catalogue::data::{CatalogueData, default_base_url};
use crate::config::provider::ProviderId;
use crate::schema::provider::{
    CustomProtocol, CustomProvider, LegacyProviderEntry, LegacyProviderEntryConfig,
    ProviderCredentials, ProviderEntry,
};

pub const MARKER: &str = "migration.provider_ids_v1";
/// `{new id: legacy name}` for the rows that were renamed, for the providers API's
/// `migrated_from`.
pub const MAP_KEY: &str = "migration.provider_ids_v1.map";

const REMOVED: &[&str] = &["galadriel", "mira"];

/// Base URLs vizier used by default before 015, which mean "no override".
fn old_default(legacy: &str) -> Option<&'static str> {
    Some(match legacy {
        "opencode_zen" => "https://opencode.ai/zen/v1",
        "opencode_go" => "https://opencode.ai/zen/go/v1",
        "moonshot" => "https://api.moonshot.ai/v1",
        "zai" => "https://api.z.ai/api/paas/v4",
        "minimax" => "https://api.minimax.io/v1",
        _ => return None,
    })
}

fn norm(url: &str) -> String {
    url.trim().trim_end_matches('/').to_ascii_lowercase()
}

/// The legacy row as credentials plus the saved base URL that decides the target.
fn split_legacy(config: LegacyProviderEntryConfig) -> (ProviderCredentials, Option<String>) {
    use LegacyProviderEntryConfig as L;
    let key = |k: String| ProviderCredentials {
        api_key: Some(k).filter(|k| !k.is_empty()),
        ..Default::default()
    };
    match config {
        L::Ollama { base_url } | L::LlamaCpp { base_url } => (
            ProviderCredentials {
                base_url: Some(base_url).filter(|u| !u.is_empty()),
                ..Default::default()
            },
            None,
        ),
        L::Chatgpt {
            access_token,
            account_id,
            base_url,
        } => (
            ProviderCredentials {
                access_token: Some(access_token).filter(|t| !t.is_empty()),
                account_id: Some(account_id).filter(|a| !a.is_empty()),
                base_url: base_url.filter(|u| !u.is_empty()),
                ..Default::default()
            },
            None,
        ),
        L::Azure { endpoint, api_key } => (
            ProviderCredentials {
                api_key: Some(api_key).filter(|k| !k.is_empty()),
                endpoint: Some(endpoint).filter(|e| !e.is_empty()),
                ..Default::default()
            },
            None,
        ),
        L::Custom { api_key, base_url } => (key(api_key), Some(base_url)),
        L::Moonshot { api_key, base_url }
        | L::Zai { api_key, base_url }
        | L::Minimax { api_key, base_url }
        | L::OpencodeZen { api_key, base_url }
        | L::OpencodeGo { api_key, base_url } => (key(api_key), base_url.filter(|u| !u.is_empty())),
        L::Openai { api_key }
        | L::Anthropic { api_key }
        | L::Deepseek { api_key }
        | L::Openrouter { api_key }
        | L::Gemini { api_key }
        | L::Mimo { api_key }
        | L::Elevenlabs { api_key }
        | L::Groq { api_key }
        | L::Mistral { api_key }
        | L::Xai { api_key }
        | L::Perplexity { api_key }
        | L::Together { api_key }
        | L::Cohere { api_key }
        | L::Huggingface { api_key }
        | L::Hyperbolic { api_key }
        | L::Voyageai { api_key }
        | L::Galadriel { api_key }
        | L::Mira { api_key }
        | L::Copilot { api_key } => (key(api_key), None),
    }
}

fn host(url: &str) -> Option<String> {
    reqwest::Url::parse(url.trim())
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
}

/// A catalogue provider on the same host (`api.minimax.cn/v1` → `minimax-cn`): the only one, or
/// among several the one whose id starts like the legacy provider's new id.
fn match_by_host(catalogue: &CatalogueData, url: &str, default_id: &str) -> Option<String> {
    let wanted = host(url)?;
    let same: Vec<&str> = catalogue
        .providers
        .values()
        .filter(|p| p.api.as_deref().and_then(host).is_some_and(|h| h == wanted))
        .map(|p| p.id.as_str())
        .collect();
    let stem: String = default_id.chars().take(4).collect();
    match same.as_slice() {
        [] => None,
        [one] => Some(one.to_string()),
        many => many
            .iter()
            .filter(|id| id.starts_with(&stem))
            .min_by_key(|id| id.len())
            .map(|id| id.to_string()),
    }
}

fn custom_provider(id: &str, name: &str, base_url: String, api_key: Option<String>) -> CustomProvider {
    CustomProvider {
        id: ProviderId::new(id.to_string()).unwrap_or(ProviderId::CUSTOM),
        name: name.to_string(),
        protocol: CustomProtocol::OpenaiCompatible,
        base_url,
        api_key,
        headers: BTreeMap::new(),
        models: vec![],
        models_fetched_at: None,
    }
}

fn upsert_custom(conn: &rusqlite::Connection, p: &CustomProvider) -> anyhow::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO custom_provider (id, data) VALUES (?1, ?2)",
        rusqlite::params![p.id.as_str(), serde_json::to_string(p)?],
    )?;
    Ok(())
}

/// Rewrite one provider id field in an agent's JSON with the deployment's map.
fn rewrite_field(slot: Option<&mut Value>, map: &HashMap<String, String>) -> Option<(String, String)> {
    let slot = slot?;
    let old = slot.as_str()?.to_string();
    let new = map
        .get(&old)
        .cloned()
        .or_else(|| ProviderId::from_legacy(&old).map(|id| id.to_string()))?;
    if new == old {
        return None;
    }
    *slot = Value::String(new.clone());
    Some((old, new))
}

pub fn migrate_provider_ids_v1(
    conn: &mut rusqlite::Connection,
    catalogue: &CatalogueData,
) -> anyhow::Result<()> {
    let done: Option<String> = conn
        .query_row(
            "SELECT value FROM state WHERE key = ?1",
            rusqlite::params![MARKER],
            |r| r.get(0),
        )
        .optional()?;
    if done.is_some() {
        return Ok(());
    }

    let tx = conn.transaction()?;
    // legacy name → new id, for this deployment.
    let mut map: HashMap<String, String> = HashMap::new();
    let mut renamed: BTreeMap<String, String> = BTreeMap::new();
    let mut had_custom_row = false;

    let rows: Vec<(String, String)> = {
        let mut stmt = tx.prepare("SELECT variant, data FROM provider_config")?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect()
    };

    for (variant, data) in rows {
        if serde_json::from_str::<ProviderEntry>(&data).is_ok() {
            continue;
        }
        let legacy: LegacyProviderEntry = match serde_json::from_str(&data) {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("provider_config row '{variant}' is unreadable and was left as it is: {e}");
                continue;
            }
        };
        let name = legacy.variant.clone();

        if REMOVED.contains(&name.as_str()) {
            tx.execute("DELETE FROM provider_config WHERE variant = ?1", rusqlite::params![variant])?;
            tracing::info!("provider '{name}' is no longer supported; its saved credentials were deleted");
            continue;
        }

        let (credentials, base_url) = split_legacy(legacy.config);
        tx.execute("DELETE FROM provider_config WHERE variant = ?1", rusqlite::params![variant])?;

        if name == "custom" {
            had_custom_row = true;
            let p = custom_provider(
                "custom",
                "Custom (OpenAI-compatible)",
                base_url.unwrap_or_default(),
                credentials.api_key,
            );
            upsert_custom(&tx, &p)?;
            map.insert(name.clone(), "custom".into());
            tracing::warn!("provider 'custom' is now the custom provider 'custom' (OpenAI-compatible, {})", p.base_url);
            continue;
        }

        let default_id = ProviderId::parse(&name)?;
        let target = match base_url.as_deref() {
            None => default_id.to_string(),
            Some(url) => {
                let wanted = norm(url);
                let matched = catalogue
                    .providers
                    .values()
                    .find(|p| p.api.as_deref().is_some_and(|a| norm(a) == wanted))
                    .map(|p| p.id.to_string())
                    .or_else(|| match_by_host(catalogue, url, default_id.as_str()));
                let is_default = old_default(&name).is_some_and(|d| norm(d) == wanted)
                    || catalogue
                        .providers
                        .get(&default_id)
                        .and_then(|p| p.api.as_deref())
                        .or_else(|| default_base_url(&default_id))
                        .is_some_and(|a| norm(a) == wanted);
                match matched {
                    Some(id) => id,
                    None if is_default => default_id.to_string(),
                    None => {
                        let id = format!("{name}-custom").replace('_', "-");
                        let p = custom_provider(&id, &format!("{name} (custom URL)"), url.to_string(), credentials.api_key.clone());
                        upsert_custom(&tx, &p)?;
                        tracing::warn!(
                            "provider '{name}' had the base URL {url}, which no catalogue provider uses; it is now the custom provider '{id}'"
                        );
                        map.insert(name.clone(), id);
                        continue;
                    }
                }
            }
        };

        let entry = ProviderEntry {
            id: ProviderId::new(target.clone())?,
            credentials,
        };
        tx.execute(
            "INSERT OR REPLACE INTO provider_config (variant, data) VALUES (?1, ?2)",
            rusqlite::params![target, serde_json::to_string(&entry)?],
        )?;
        if target != name {
            tracing::info!("provider '{name}' → '{target}'");
            renamed.insert(target.clone(), name.clone());
        }
        map.insert(name, target);
    }

    if !had_custom_row
        && let Some(base_url) = std::env::var("CUSTOM_BASE_URL").ok().filter(|u| !u.is_empty())
    {
        let exists: Option<String> = tx
            .query_row("SELECT id FROM custom_provider WHERE id = 'custom'", [], |r| r.get(0))
            .optional()?;
        if exists.is_none() {
            let key = std::env::var("CUSTOM_API_KEY").ok().filter(|k| !k.is_empty());
            upsert_custom(&tx, &custom_provider("custom", "Custom (OpenAI-compatible)", base_url, key))?;
            tracing::warn!("CUSTOM_BASE_URL is now the custom provider 'custom'");
        }
    }

    let agents: Vec<(String, String)> = {
        let mut stmt = tx.prepare("SELECT agent_id, data FROM agent_config")?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect()
    };
    for (agent_id, data) in agents {
        let Ok(mut value) = serde_json::from_str::<Value>(&data) else {
            tracing::warn!("agent '{agent_id}' has an unreadable config; its provider was not migrated");
            continue;
        };
        let mut changes = vec![];
        changes.extend(rewrite_field(value.get_mut("provider"), &map));
        changes.extend(rewrite_field(value.get_mut("dream_provider"), &map));
        changes.extend(rewrite_field(
            value
                .get_mut("tools")
                .and_then(|t| t.get_mut("read_image"))
                .and_then(|r| r.get_mut("settings"))
                .and_then(|s| s.get_mut("provider")),
            &map,
        ));
        if changes.is_empty() {
            continue;
        }
        tx.execute(
            "UPDATE agent_config SET data = ?1 WHERE agent_id = ?2",
            rusqlite::params![serde_json::to_string(&value)?, agent_id],
        )?;
        for (old, new) in changes {
            tracing::info!("agent '{agent_id}': provider '{old}' → '{new}'");
        }
    }

    tx.execute(
        "INSERT OR REPLACE INTO state (key, value) VALUES (?1, ?2)",
        rusqlite::params![MAP_KEY, serde_json::to_string(&renamed)?],
    )?;
    tx.execute(
        "INSERT OR REPLACE INTO state (key, value) VALUES (?1, ?2)",
        rusqlite::params![MARKER, "true"],
    )?;
    tx.commit()?;
    Ok(())
}

/// `{new id: legacy name}` recorded by the migration.
pub fn migrated_from(conn: &rusqlite::Connection) -> BTreeMap<String, String> {
    conn.query_row(
        "SELECT value FROM state WHERE key = ?1",
        rusqlite::params![MAP_KEY],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .and_then(|v| serde_json::from_str(&v).ok())
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::data::parse;

    fn catalogue() -> CatalogueData {
        parse(
            br#"{
          "google": {"id": "google", "name": "Google", "npm": "@ai-sdk/google", "models": {}},
          "moonshotai": {"id": "moonshotai", "name": "M", "npm": "@ai-sdk/openai-compatible", "api": "https://api.moonshot.ai/v1", "models": {}},
          "moonshotai-cn": {"id": "moonshotai-cn", "name": "M CN", "npm": "@ai-sdk/openai-compatible", "api": "https://api.moonshot.cn/v1", "models": {}},
          "zhipuai": {"id": "zhipuai", "name": "Z", "npm": "@ai-sdk/openai-compatible", "api": "https://open.bigmodel.cn/api/paas/v4", "models": {}},
          "zai": {"id": "zai", "name": "Z.ai", "npm": "@ai-sdk/openai-compatible", "api": "https://api.z.ai/api/paas/v4", "models": {}},
          "minimax-cn": {"id": "minimax-cn", "name": "MM", "npm": "@ai-sdk/anthropic", "api": "https://api.minimax.cn/anthropic/v1", "models": {}},
          "opencode": {"id": "opencode", "name": "O", "npm": "@ai-sdk/openai-compatible", "api": "https://opencode.ai/inference/v1", "models": {}}
        }"#,
        )
        .unwrap()
    }

    fn db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE state (key TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE provider_config (variant TEXT PRIMARY KEY, data TEXT NOT NULL);
             CREATE TABLE custom_provider (id TEXT PRIMARY KEY, data TEXT NOT NULL);
             CREATE TABLE agent_config (agent_id TEXT PRIMARY KEY, data TEXT NOT NULL);",
        )
        .unwrap();
        conn
    }

    fn legacy(conn: &rusqlite::Connection, variant: &str, ty: &str, data: Value) {
        let row = serde_json::json!({"variant": variant, "config": {"type": ty, "data": data}});
        conn.execute(
            "INSERT INTO provider_config (variant, data) VALUES (?1, ?2)",
            rusqlite::params![variant, row.to_string()],
        )
        .unwrap();
    }

    fn agent(conn: &rusqlite::Connection, id: &str, provider: &str, dream: Option<&str>, read_image: Option<&str>) {
        let v = serde_json::json!({
            "name": id, "provider": provider, "model": "m", "dream_provider": dream,
            "tools": {"timeout": "30s", "read_image": {"enabled": true, "settings": {"provider": read_image, "model": null}}}
        });
        conn.execute(
            "INSERT INTO agent_config (agent_id, data) VALUES (?1, ?2)",
            rusqlite::params![id, v.to_string()],
        )
        .unwrap();
    }

    fn provider(conn: &rusqlite::Connection, id: &str) -> Option<ProviderEntry> {
        conn.query_row(
            "SELECT data FROM provider_config WHERE variant = ?1",
            rusqlite::params![id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .unwrap()
        .map(|d| serde_json::from_str(&d).unwrap())
    }

    fn custom(conn: &rusqlite::Connection, id: &str) -> Option<CustomProvider> {
        conn.query_row(
            "SELECT data FROM custom_provider WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .unwrap()
        .map(|d| serde_json::from_str(&d).unwrap())
    }

    fn agent_json(conn: &rusqlite::Connection, id: &str) -> Value {
        let d: String = conn
            .query_row("SELECT data FROM agent_config WHERE agent_id = ?1", rusqlite::params![id], |r| r.get(0))
            .unwrap();
        serde_json::from_str(&d).unwrap()
    }

    #[test]
    fn rewrites_every_row_of_the_name_map_and_both_footnotes() {
        let mut conn = db();
        let simple = [
            ("openai", "Openai"), ("anthropic", "Anthropic"), ("openrouter", "Openrouter"),
            ("deepseek", "Deepseek"), ("gemini", "Gemini"), ("mimo", "Mimo"), ("groq", "Groq"),
            ("mistral", "Mistral"), ("xai", "Xai"), ("perplexity", "Perplexity"),
            ("together", "Together"), ("cohere", "Cohere"), ("huggingface", "Huggingface"),
            ("copilot", "Copilot"), ("elevenlabs", "Elevenlabs"), ("voyageai", "Voyageai"),
            ("hyperbolic", "Hyperbolic"), ("galadriel", "Galadriel"), ("mira", "Mira"),
        ];
        for (variant, ty) in simple {
            legacy(&conn, variant, ty, serde_json::json!({"api_key": format!("k-{variant}")}));
        }
        legacy(&conn, "ollama", "Ollama", serde_json::json!({"base_url": "http://gpu:11434"}));
        legacy(&conn, "llama_cpp", "LlamaCpp", serde_json::json!({"base_url": "http://cpp:8080"}));
        legacy(&conn, "chatgpt", "Chatgpt", serde_json::json!({"access_token": "t", "account_id": "a"}));
        legacy(&conn, "azure", "Azure", serde_json::json!({"endpoint": "https://r.openai.azure.com", "api_key": "k"}));
        // ¹ base URLs: a catalogue match, an old default, and no match.
        legacy(&conn, "moonshot", "Moonshot", serde_json::json!({"api_key": "km", "base_url": "https://api.moonshot.cn/v1/"}));
        legacy(&conn, "zai", "Zai", serde_json::json!({"api_key": "kz", "base_url": "https://open.bigmodel.cn/api/paas/v4"}));
        legacy(&conn, "minimax", "Minimax", serde_json::json!({"api_key": "kmm", "base_url": "https://api.minimax.cn/v1"}));
        legacy(&conn, "opencode_zen", "OpencodeZen", serde_json::json!({"api_key": "ko", "base_url": "https://opencode.ai/zen/v1"}));
        legacy(&conn, "opencode_go", "OpencodeGo", serde_json::json!({"api_key": "kg", "base_url": "https://proxy.example/go/v1"}));
        // ² the legacy custom provider.
        legacy(&conn, "custom", "Custom", serde_json::json!({"api_key": "kc", "base_url": "http://127.0.0.1:18080/v1"}));

        agent(&conn, "gem-old", "gemini", Some("together"), Some("copilot"));
        agent(&conn, "moon-old", "moonshot", None, None);
        agent(&conn, "cus-old", "custom", None, None);
        agent(&conn, "gal-old", "galadriel", None, None);
        agent(&conn, "go-old", "opencode_go", None, None);
        agent(&conn, "new", "anthropic", None, None);

        migrate_provider_ids_v1(&mut conn, &catalogue()).unwrap();

        for (old, new) in [
            ("openai", "openai"), ("anthropic", "anthropic"), ("gemini", "google"), ("mimo", "xiaomi"),
            ("together", "togetherai"), ("copilot", "github-copilot"), ("llama_cpp", "llama-cpp"),
            ("moonshot", "moonshotai-cn"), ("zai", "zhipuai"), ("minimax", "minimax-cn"), ("opencode_zen", "opencode"),
        ] {
            let entry = provider(&conn, new).unwrap_or_else(|| panic!("{old} → {new} missing"));
            assert_eq!(entry.id.as_str(), new);
            if old != new {
                assert!(provider(&conn, old).is_none(), "{old} left behind");
            }
        }
        assert_eq!(provider(&conn, "google").unwrap().credentials.api_key.as_deref(), Some("k-gemini"));
        assert_eq!(provider(&conn, "ollama").unwrap().credentials.base_url.as_deref(), Some("http://gpu:11434"));
        assert_eq!(provider(&conn, "chatgpt").unwrap().credentials.account_id.as_deref(), Some("a"));
        assert_eq!(provider(&conn, "azure").unwrap().credentials.endpoint.as_deref(), Some("https://r.openai.azure.com"));
        assert!(provider(&conn, "galadriel").is_none() && provider(&conn, "mira").is_none());

        let go = custom(&conn, "opencode-go-custom").unwrap();
        assert_eq!(go.base_url, "https://proxy.example/go/v1");
        assert_eq!(go.api_key.as_deref(), Some("kg"));
        let c = custom(&conn, "custom").unwrap();
        assert_eq!((c.base_url.as_str(), c.protocol), ("http://127.0.0.1:18080/v1", CustomProtocol::OpenaiCompatible));

        let gem = agent_json(&conn, "gem-old");
        assert_eq!(gem["provider"], "google");
        assert_eq!(gem["dream_provider"], "togetherai");
        assert_eq!(gem["tools"]["read_image"]["settings"]["provider"], "github-copilot");
        assert_eq!(agent_json(&conn, "moon-old")["provider"], "moonshotai-cn");
        assert_eq!(agent_json(&conn, "cus-old")["provider"], "custom");
        assert_eq!(agent_json(&conn, "gal-old")["provider"], "galadriel", "left to fail at build");
        assert_eq!(agent_json(&conn, "go-old")["provider"], "opencode-go-custom");

        let from = migrated_from(&conn);
        assert_eq!(from.get("google").map(String::as_str), Some("gemini"));
        assert_eq!(from.get("moonshotai-cn").map(String::as_str), Some("moonshot"));
    }

    #[test]
    fn is_idempotent() {
        let mut conn = db();
        legacy(&conn, "gemini", "Gemini", serde_json::json!({"api_key": "k"}));
        migrate_provider_ids_v1(&mut conn, &catalogue()).unwrap();
        // A row written afterwards in the legacy shape is not touched again: the marker is set.
        legacy(&conn, "groq", "Groq", serde_json::json!({"api_key": "k"}));
        migrate_provider_ids_v1(&mut conn, &catalogue()).unwrap();
        assert!(provider(&conn, "google").is_some());
        let raw: String = conn
            .query_row("SELECT data FROM provider_config WHERE variant = 'groq'", [], |r| r.get(0))
            .unwrap();
        assert!(raw.contains("\"Groq\""));
    }
}
