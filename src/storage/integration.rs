use anyhow::Result;

use crate::{
    schema::{AgentId, EventDraft, Integration, IntegrationEvent, IntegrationFamily, IntegrationGrant},
    storage::VizierStorage,
};

/// A write refused because it would break a per-agent or install-wide rule (data-model §1–2).
/// Returned through `anyhow` and downcast by the HTTP layer, which answers `409` with
/// [`IntegrationConflict::code`].
#[derive(Debug, Clone, PartialEq)]
pub enum IntegrationConflict {
    /// Another connection of the same platform already uses this bot token (FR-009).
    TokenInUse { integration_id: String, name: String },
    /// The agent already has a connection for this platform.
    PlatformInUseOnAgent { integration_id: String, name: String },
    /// The agent already has an MCP server with this name.
    NameInUseOnAgent { integration_id: String, name: String },
    /// The agent already has a configuration of this addon; edit that one instead.
    AddonInstalled { integration_id: String, name: String },
}

impl IntegrationConflict {
    pub fn code(&self) -> &'static str {
        match self {
            Self::TokenInUse { .. } => "token_in_use",
            Self::PlatformInUseOnAgent { .. } => "platform_in_use_on_agent",
            Self::NameInUseOnAgent { .. } => "name_in_use_on_agent",
            Self::AddonInstalled { .. } => "addon_installed",
        }
    }

    /// The integration already holding what was asked for.
    pub fn conflicting(&self) -> (&str, &str) {
        match self {
            Self::TokenInUse { integration_id, name }
            | Self::PlatformInUseOnAgent { integration_id, name }
            | Self::NameInUseOnAgent { integration_id, name }
            | Self::AddonInstalled { integration_id, name } => (integration_id, name),
        }
    }
}

impl std::fmt::Display for IntegrationConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (_, name) = self.conflicting();
        match self {
            Self::TokenInUse { .. } => write!(f, "this bot token is already used by '{name}'"),
            Self::PlatformInUseOnAgent { .. } => {
                write!(f, "the agent already has a connection for this platform ('{name}')")
            }
            Self::NameInUseOnAgent { .. } => {
                write!(f, "the agent already has an MCP server named '{name}'")
            }
            Self::AddonInstalled { integration_id, .. } => write!(
                f,
                "this addon is already installed as '{name}' ({integration_id}); edit that one instead"
            ),
        }
    }
}

impl std::error::Error for IntegrationConflict {}

#[async_trait::async_trait]
pub trait IntegrationStorage {
    async fn list_integrations(&self) -> Result<Vec<Integration>>;

    async fn list_agent_integrations(&self, agent_id: &str) -> Result<Vec<Integration>>;

    async fn get_integration(&self, id: &str) -> Result<Option<Integration>>;

    /// Upsert, recording each of `events` for every agent it is linked to, in one transaction.
    /// `attach_to` creates it on that agent (refused if the id already exists): an integration
    /// is linked to exactly one agent, once, when it is created; an update never changes links.
    /// Enforces channel token uniqueness, one connection per platform per agent, one
    /// configuration per addon per agent and MCP names unique per agent. Returns the agents
    /// it is linked to. `integration.agents` is ignored.
    async fn save_integration(
        &self,
        integration: &Integration,
        attach_to: Option<&str>,
        events: &[EventDraft],
    ) -> Result<Vec<AgentId>>;

    /// Records `event` for every agent it was linked to. Returns those agents.
    async fn delete_integration(&self, id: &str, event: &EventDraft) -> Result<Vec<AgentId>>;

    /// Remove one agent's link to a legacy integration linked to several (FR-023). Refused
    /// for an integration linked to fewer than two agents: delete it instead.
    async fn unlink(&self, id: &str, agent_id: &str, event: &EventDraft) -> Result<Vec<AgentId>>;

    /// For a deleted agent: the integrations linked only to it are deleted, its links to legacy
    /// ones removed, and its grants and events dropped. Returns the deleted integrations' ids.
    async fn delete_agent_integrations(&self, agent_id: &str) -> Result<Vec<String>>;

    async fn list_grants(&self, agent_id: &str) -> Result<Vec<IntegrationGrant>>;

    /// Switch one family's self-management on or off, recording `grant_on`/`grant_off`.
    /// Returns whether anything changed; an unchanged switch records nothing.
    async fn set_grant(
        &self,
        agent_id: &str,
        family: IntegrationFamily,
        on: bool,
        granted_by: &str,
    ) -> Result<bool>;

    /// Newest first, keyed on `(at, id)`: `before` is the last row of the previous page.
    async fn list_events(
        &self,
        agent_id: &str,
        integration_id: Option<&str>,
        before: Option<(chrono::DateTime<chrono::Utc>, String)>,
        limit: usize,
    ) -> Result<Vec<IntegrationEvent>>;
}

#[async_trait::async_trait]
impl IntegrationStorage for VizierStorage {
    async fn list_integrations(&self) -> Result<Vec<Integration>> {
        self.0.list_integrations().await
    }

    async fn list_agent_integrations(&self, agent_id: &str) -> Result<Vec<Integration>> {
        self.0.list_agent_integrations(agent_id).await
    }

    async fn get_integration(&self, id: &str) -> Result<Option<Integration>> {
        self.0.get_integration(id).await
    }

    async fn save_integration(
        &self,
        integration: &Integration,
        attach_to: Option<&str>,
        events: &[EventDraft],
    ) -> Result<Vec<AgentId>> {
        self.0.save_integration(integration, attach_to, events).await
    }

    async fn delete_integration(&self, id: &str, event: &EventDraft) -> Result<Vec<AgentId>> {
        self.0.delete_integration(id, event).await
    }

    async fn unlink(&self, id: &str, agent_id: &str, event: &EventDraft) -> Result<Vec<AgentId>> {
        self.0.unlink(id, agent_id, event).await
    }

    async fn delete_agent_integrations(&self, agent_id: &str) -> Result<Vec<String>> {
        self.0.delete_agent_integrations(agent_id).await
    }

    async fn list_grants(&self, agent_id: &str) -> Result<Vec<IntegrationGrant>> {
        self.0.list_grants(agent_id).await
    }

    async fn set_grant(
        &self,
        agent_id: &str,
        family: IntegrationFamily,
        on: bool,
        granted_by: &str,
    ) -> Result<bool> {
        self.0.set_grant(agent_id, family, on, granted_by).await
    }

    async fn list_events(
        &self,
        agent_id: &str,
        integration_id: Option<&str>,
        before: Option<(chrono::DateTime<chrono::Utc>, String)>,
        limit: usize,
    ) -> Result<Vec<IntegrationEvent>> {
        self.0.list_events(agent_id, integration_id, before, limit).await
    }
}
