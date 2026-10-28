//! Integrations (specs/016-integrations-section): channel connections, addons and MCP servers,
//! stored once and attached to agents. Kind-specific behaviour lives behind
//! [`kind::IntegrationKind`]; this module holds the registry, the per-agent integration tool
//! layer and the shared helpers.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};

use crate::{
    agents::{
        mcp::McpServer,
        tools::{AddonTools, VizierToolSet},
    },
    schema::{Integration, IntegrationFamily, IntegrationStatus},
};

pub mod access;
pub mod kind;
pub mod kinds;
pub mod service;

use kind::{AgentToolContext, Contribution, IntegrationKind};

pub use crate::utils::{MASK, mask_secret};

/// Every integration kind the binary knows, registered in one place.
pub struct IntegrationKinds {
    kinds: Vec<Arc<dyn IntegrationKind>>,
}

impl IntegrationKinds {
    pub fn builtin() -> Self {
        Self {
            kinds: vec![
                Arc::new(kinds::discord::DiscordKind),
                Arc::new(kinds::telegram::TelegramKind),
                Arc::new(kinds::brave_search::BraveSearchKind),
                Arc::new(kinds::mcp::McpKind),
            ],
        }
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn IntegrationKind>> {
        self.kinds.iter().find(|kind| kind.id() == id).cloned()
    }

    pub fn by_family(&self, family: Option<IntegrationFamily>) -> Vec<Arc<dyn IntegrationKind>> {
        self.kinds
            .iter()
            .filter(|kind| family.is_none_or(|f| kind.family() == f))
            .cloned()
            .collect()
    }

    /// Category names reserved for addons, attached or not.
    pub fn addon_ids(&self) -> Vec<String> {
        self.by_family(Some(IntegrationFamily::Addon))
            .iter()
            .map(|kind| kind.id().to_string())
            .collect()
    }
}

impl crate::encryption::IntegrationSecretPaths for IntegrationKinds {
    /// The secret paths declared by a kind's listing: a `Secret` field is the top-level key, a
    /// `SecretMap` is `<key>.*` (every value; names stay readable). `None` for an unknown kind,
    /// which makes storage protect the whole settings blob.
    fn secret_paths(&self, kind: &str) -> Option<Vec<String>> {
        let kind = self.get(kind)?;
        Some(
            kind.listing()
                .fields
                .into_iter()
                .filter_map(|field| match field.field_type {
                    kind::FieldType::Secret => Some(field.key.to_string()),
                    kind::FieldType::SecretMap => Some(format!("{}.*", field.key)),
                    _ => None,
                })
                .collect(),
        )
    }
}

/// Addon errors by integration id, written by addon tools (an auth failure) and cleared on
/// their next success (research R9).
#[derive(Clone, Default)]
pub struct IntegrationHealth(Arc<RwLock<HashMap<String, String>>>);

impl IntegrationHealth {
    pub fn fail(&self, integration_id: &str, error: String) {
        self.0
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(integration_id.to_string(), error);
    }

    pub fn clear(&self, integration_id: &str) {
        self.0.write().unwrap_or_else(|p| p.into_inner()).remove(integration_id);
    }

    pub fn get(&self, integration_id: &str) -> Option<String> {
        self.0.read().unwrap_or_else(|p| p.into_inner()).get(integration_id).cloned()
    }
}

/// What a contribution was built from: rebuilt only when this changes.
type Fingerprint = (String, String, serde_json::Value);

#[derive(Clone)]
struct Entry {
    fingerprint: Fingerprint,
    contribution: Contribution,
}

/// The swappable half of an agent's tools: what its attached integrations contribute
/// (research R2). Rebuilt on every integration change, carrying over unchanged contributions —
/// and with them their `McpServer` connections — from the previous one.
#[derive(Clone, Default)]
pub struct IntegrationTools {
    entries: HashMap<String, Entry>,
    pub native: VizierToolSet,
    pub addons: BTreeMap<String, AddonTools>,
    pub mcp: HashMap<String, Arc<McpServer>>,
    /// The agent's self-management tools, present only while a family is granted
    /// (specs/017-agent-self-integrations research R5). Rebuilt on every refresh; cheap.
    pub self_management: VizierToolSet,
}

impl IntegrationTools {
    /// Contributions of the enabled, complete integrations among `integrations`.
    pub fn build(
        kinds: &IntegrationKinds,
        integrations: &[Integration],
        ctx: &AgentToolContext,
        previous: Option<&IntegrationTools>,
    ) -> Self {
        let mut tools = Self::default();
        let mut ordered: Vec<&Integration> = integrations.iter().filter(|i| i.enabled).collect();
        ordered.sort_by(|a, b| a.id.cmp(&b.id));

        for integration in ordered {
            let Some(kind) = kinds.get(&integration.kind) else {
                tracing::warn!(
                    integration = %integration.id,
                    kind = %integration.kind,
                    "unknown integration kind, ignored"
                );
                continue;
            };
            if !kind.missing(&integration.settings).is_empty() {
                continue;
            }
            let fingerprint = (
                integration.kind.clone(),
                integration.name.clone(),
                integration.settings.clone(),
            );
            let reused = previous
                .and_then(|p| p.entries.get(&integration.id))
                .filter(|entry| entry.fingerprint == fingerprint)
                .map(|entry| entry.contribution.clone());
            let contribution = match reused {
                Some(contribution) => contribution,
                None => {
                    let contribution = kind.contribute(integration, ctx);
                    // In the background: no turn waits on a connect (#53).
                    if let Some((_, server)) = &contribution.mcp {
                        server.warm_up();
                    }
                    contribution
                }
            };

            match contribution.addon {
                Some(addon) => {
                    let entry = tools.addons.entry(addon.to_string()).or_insert_with(|| AddonTools {
                        description: kind.listing().description.to_string(),
                        toolset: VizierToolSet::new(),
                    });
                    entry.toolset.tools.extend(contribution.native.tools.clone());
                }
                None => tools.native.tools.extend(contribution.native.tools.clone()),
            }
            if let Some((name, server)) = &contribution.mcp {
                tools.mcp.insert(name.clone(), server.clone());
            }
            tools
                .entries
                .insert(integration.id.clone(), Entry { fingerprint, contribution });
        }
        tools
    }

    /// The MCP server an integration contributes to this agent, if it is in use.
    pub fn mcp_server(&self, integration_id: &str) -> Option<&Arc<McpServer>> {
        self.entries
            .get(integration_id)
            .and_then(|entry| entry.contribution.mcp.as_ref())
            .map(|(_, server)| server)
    }
}

/// One agent's view of an integration attached to it, from the agent's own snapshot and the
/// addon health map. Channel connections are answered by `ChannelConnections` instead.
pub fn agent_status(
    kind: &dyn IntegrationKind,
    integration: &Integration,
    snapshot: &IntegrationTools,
    health: &IntegrationHealth,
) -> IntegrationStatus {
    if !integration.enabled {
        return IntegrationStatus::Disabled;
    }
    let missing = kind.missing(&integration.settings);
    if !missing.is_empty() {
        return IntegrationStatus::Incomplete { missing };
    }
    if let Some(server) = snapshot.mcp_server(&integration.id) {
        return server.status();
    }
    match health.get(&integration.id) {
        Some(last_error) => IntegrationStatus::Failed { last_error },
        None if snapshot.entries.contains_key(&integration.id) => {
            IntegrationStatus::Running { tools: None }
        }
        // Saved after the snapshot was composed; the refresh is on its way.
        None => IntegrationStatus::Connecting,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use parking_lot::Mutex;
    use serde_json::json;

    use super::*;
    use crate::{
        agents::tools::{BaseTools, compose},
        storage::{VizierStorage, document::LocalDocumentStore, sqlite::SqliteStorage},
    };

    fn ctx() -> AgentToolContext {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        AgentToolContext {
            agent_id: "dp-a".into(),
            storage: Arc::new(VizierStorage::new(SqliteStorage::new(
                Arc::new(Mutex::new(conn)),
                Arc::new(LocalDocumentStore::new(std::env::temp_dir())),
            ))),
            health: IntegrationHealth::default(),
        }
    }

    fn integration(id: &str, kind: &str, family: IntegrationFamily, name: &str, settings: serde_json::Value) -> Integration {
        Integration {
            id: id.into(),
            kind: kind.into(),
            family,
            name: name.into(),
            settings,
            enabled: true,
            creator: None,
            agents: vec!["dp-a".into()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn fs(args: &[&str]) -> Integration {
        integration(
            "mcp-1",
            "mcp",
            IntegrationFamily::Mcp,
            "fs",
            json!({"transport": "local", "command": "vizier-test-no-such-command", "args": args}),
        )
    }

    fn brave(key: &str) -> Integration {
        integration("brave-1", "brave_search", IntegrationFamily::Addon, "brave", json!({"api_key": key}))
    }

    #[tokio::test]
    async fn an_unrelated_change_keeps_the_mcp_connection() {
        let kinds = IntegrationKinds::builtin();
        let ctx = ctx();
        let first = IntegrationTools::build(&kinds, &[fs(&["a"]), brave("one")], &ctx, None);
        let second =
            IntegrationTools::build(&kinds, &[fs(&["a"]), brave("two")], &ctx, Some(&first));
        assert!(Arc::ptr_eq(&first.mcp["fs"], &second.mcp["fs"]));
        assert_eq!(second.addons["brave_search"].toolset.tools.len(), 2);
    }

    #[tokio::test]
    async fn a_changed_server_is_replaced() {
        let kinds = IntegrationKinds::builtin();
        let ctx = ctx();
        let first = IntegrationTools::build(&kinds, &[fs(&["a"])], &ctx, None);
        let second = IntegrationTools::build(&kinds, &[fs(&["b"])], &ctx, Some(&first));
        assert!(!Arc::ptr_eq(&first.mcp["fs"], &second.mcp["fs"]));

        let gone = IntegrationTools::build(&kinds, &[], &ctx, Some(&second));
        assert!(gone.mcp.is_empty());
    }

    #[tokio::test]
    async fn disabled_and_incomplete_integrations_contribute_nothing() {
        let kinds = IntegrationKinds::builtin();
        let mut off = brave("key");
        off.enabled = false;
        let tools = IntegrationTools::build(&kinds, &[off, brave("")], &ctx(), None);
        assert!(tools.addons.is_empty());
    }

    /// The encryption paths come from the same listing that redaction uses: a `Secret` field is
    /// its key, a `SecretMap` is `<key>.*`, and everything else is not secret.
    #[test]
    fn secret_paths_match_each_kind_listing() {
        use crate::encryption::IntegrationSecretPaths;

        let kinds = IntegrationKinds::builtin();
        assert_eq!(kinds.secret_paths("discord").unwrap(), ["token"]);
        assert_eq!(kinds.secret_paths("telegram").unwrap(), ["token"]);
        assert_eq!(kinds.secret_paths("brave_search").unwrap(), ["api_key"]);
        assert_eq!(kinds.secret_paths("mcp").unwrap(), ["env.*"]);
        assert_eq!(kinds.secret_paths("retired_kind"), None);

        for kind in kinds.by_family(None) {
            let paths = kinds.secret_paths(kind.id()).unwrap();
            for field in kind.listing().fields {
                match field.field_type {
                    kind::FieldType::Secret => assert!(paths.contains(&field.key.to_string())),
                    kind::FieldType::SecretMap => {
                        assert!(paths.contains(&format!("{}.*", field.key)))
                    }
                    _ => assert!(!paths.iter().any(|p| p == field.key)),
                }
            }
        }
    }

    #[tokio::test]
    async fn identical_inputs_compose_identical_categories() {        let kinds = IntegrationKinds::builtin();
        let ctx = ctx();
        let base = BaseTools::empty_for_tests(kinds.addon_ids());
        let integrations = [fs(&["a"]), brave("key")];
        let a = compose(&base, Arc::new(IntegrationTools::build(&kinds, &integrations, &ctx, None)));
        let b = compose(&base, Arc::new(IntegrationTools::build(&kinds, &integrations, &ctx, None)));
        assert_eq!(a.categories, b.categories);
        let names: Vec<&str> = a.categories.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["brave_search", "fs"]);
    }

    /// Research R12: every tool an addon contributes is named `<addon id>_…`.
    #[tokio::test]
    async fn every_addon_tool_is_namespaced_by_its_addon() {
        let kinds = IntegrationKinds::builtin();
        for kind in kinds.by_family(Some(IntegrationFamily::Addon)) {
            let settings: serde_json::Map<String, serde_json::Value> = kind
                .listing()
                .fields
                .iter()
                .filter(|field| field.required)
                .map(|field| (field.key.to_string(), json!("placeholder-value")))
                .collect();
            let addon = integration("x", kind.id(), kind.family(), "x", settings.into());
            let contribution = kind.contribute(&addon, &ctx());
            assert_eq!(contribution.addon, Some(kind.id()));
            assert!(!contribution.native.tools.is_empty(), "{} contributes nothing", kind.id());
            for name in contribution.native.tools.keys() {
                assert!(name.starts_with(&format!("{}_", kind.id())), "{name}");
            }
        }
    }
}
