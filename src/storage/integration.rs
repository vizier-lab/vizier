use anyhow::Result;

use crate::{
    schema::{AgentId, Integration},
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
    /// The agent already has an MCP server with this name attached.
    NameInUseOnAgent { integration_id: String, name: String },
    /// A channel connection belongs to the agent it was created on: it can't be detached or
    /// attached elsewhere, only deleted.
    ChannelBelongsToAgent { integration_id: String, name: String },
}

impl IntegrationConflict {
    pub fn code(&self) -> &'static str {
        match self {
            Self::TokenInUse { .. } => "token_in_use",
            Self::PlatformInUseOnAgent { .. } => "platform_in_use_on_agent",
            Self::NameInUseOnAgent { .. } => "name_in_use_on_agent",
            Self::ChannelBelongsToAgent { .. } => "channel_belongs_to_agent",
        }
    }

    /// The integration already holding what was asked for.
    pub fn conflicting(&self) -> (&str, &str) {
        match self {
            Self::TokenInUse { integration_id, name }
            | Self::PlatformInUseOnAgent { integration_id, name }
            | Self::NameInUseOnAgent { integration_id, name }
            | Self::ChannelBelongsToAgent { integration_id, name } => (integration_id, name),
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
                write!(f, "the agent already has an MCP server named '{name}' attached")
            }
            Self::ChannelBelongsToAgent { .. } => write!(
                f,
                "'{name}' belongs to the agent it was created on; delete it and connect the bot on the other agent instead"
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

    /// Upsert, and when `attach_to` is set attach it to that agent, in one transaction.
    /// Enforces channel token uniqueness and every attachment rule `set_attachment` does
    /// (for the agents it ends up attached to). Returns the affected agents: attached before
    /// ∪ attached after. `integration.agents` is ignored.
    async fn save_integration(
        &self,
        integration: &Integration,
        attach_to: Option<&str>,
    ) -> Result<Vec<AgentId>>;

    /// Returns the agents it was attached to.
    async fn delete_integration(&self, id: &str) -> Result<Vec<AgentId>>;

    /// Attaching an addon configuration replaces any other configuration of the same kind on
    /// that agent. A channel connection belongs to its agent for good: detaching it, or
    /// attaching it to another agent, is refused. Returns the affected agents.
    async fn set_attachment(
        &self,
        id: &str,
        agent_id: &str,
        attached: bool,
    ) -> Result<Vec<AgentId>>;

    /// For a deleted agent: its channel connections are deleted with it, and its attachments
    /// to everything else removed. Returns the deleted connections' ids.
    async fn delete_agent_integrations(&self, agent_id: &str) -> Result<Vec<String>>;
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
    ) -> Result<Vec<AgentId>> {
        self.0.save_integration(integration, attach_to).await
    }

    async fn delete_integration(&self, id: &str) -> Result<Vec<AgentId>> {
        self.0.delete_integration(id).await
    }

    async fn set_attachment(
        &self,
        id: &str,
        agent_id: &str,
        attached: bool,
    ) -> Result<Vec<AgentId>> {
        self.0.set_attachment(id, agent_id, attached).await
    }

    async fn delete_agent_integrations(&self, agent_id: &str) -> Result<Vec<String>> {
        self.0.delete_agent_integrations(agent_id).await
    }
}
