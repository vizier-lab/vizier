//! Integrations (specs/016-integrations-section, specs/017-agent-self-integrations): channel
//! connections, addon configurations and MCP servers. Each belongs to exactly one agent.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::schema::{AgentId, Requester};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationFamily {
    Channel,
    Addon,
    Mcp,
}

impl IntegrationFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Channel => "channel",
            Self::Addon => "addon",
            Self::Mcp => "mcp",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "channel" => Self::Channel,
            "addon" => Self::Addon,
            "mcp" => Self::Mcp,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Integration {
    pub id: String,
    /// An `IntegrationKind::id()`.
    pub kind: String,
    /// The kind's family, stored so per-agent rules can be enforced inside the storage
    /// transaction without the kinds registry.
    pub family: IntegrationFamily,
    /// Display name; for `mcp` also the server key in tool names.
    pub name: String,
    /// Shape set by the kind. Secrets are stored in clear, like `provider_config`.
    pub settings: serde_json::Value,
    pub enabled: bool,
    /// Who made it: a person or the agent itself. Informational; grants nothing.
    pub creator: Option<Requester>,
    /// The agents it is linked to, from `integration_attachment`: exactly one for anything
    /// created since 017; more than one, or none, only for rows left over from 016.
    #[serde(default)]
    pub agents: Vec<AgentId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Integration {
    /// The agent it belongs to, unless it is a legacy row linked to several agents or none.
    pub fn owner_agent(&self) -> Option<&AgentId> {
        match self.agents.as_slice() {
            [agent] => Some(agent),
            _ => None,
        }
    }

    /// Left over from 016's many-to-many attachments (FR-023): only all-agents managers may
    /// change it, and agents never see it in their self-management tools.
    pub fn is_legacy(&self) -> bool {
        self.agents.len() != 1
    }
}

/// One agent's view of one integration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum IntegrationStatus {
    Disabled,
    /// A required setting is missing.
    Incomplete { missing: Vec<String> },
    Connecting,
    /// Reader up, MCP connected, or addon ready.
    Running {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tools: Option<usize>,
    },
    Reconnecting {
        last_error: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next_retry_at: Option<DateTime<Utc>>,
    },
    /// A fatal channel error, or an addon's auth error.
    Failed { last_error: String },
    /// This agent's connection to an MCP server is down.
    Unreachable { last_error: String },
    /// The agent isn't running, so nothing can be said.
    Unknown,
}

/// Published after an integration write commits (contracts/runtime.md). Carries no settings:
/// subscribers re-read storage. A self-management grant change publishes one with an empty
/// `integration_id`, so the agent recomposes its tools like for any other change.
#[derive(Debug, Clone)]
pub struct IntegrationChanged {
    pub integration_id: String,
    pub family: IntegrationFamily,
    /// Linked before ∪ linked after the change.
    pub agents: Vec<AgentId>,
}

/// One integration family an agent may manage itself (research R1). A row means the switch is on.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IntegrationGrant {
    pub agent_id: AgentId,
    pub family: IntegrationFamily,
    pub granted_by: String,
    pub granted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventAction {
    Created,
    Edited,
    Enabled,
    Disabled,
    Deleted,
    Unlinked,
    GrantOn,
    GrantOff,
}

impl EventAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Edited => "edited",
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::Deleted => "deleted",
            Self::Unlinked => "unlinked",
            Self::GrantOn => "grant_on",
            Self::GrantOff => "grant_off",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "created" => Self::Created,
            "edited" => Self::Edited,
            "enabled" => Self::Enabled,
            "disabled" => Self::Disabled,
            "deleted" => Self::Deleted,
            "unlinked" => Self::Unlinked,
            "grant_on" => Self::GrantOn,
            "grant_off" => Self::GrantOff,
            _ => return None,
        })
    }
}

/// One recorded change to an agent's integrations or grants (research R6). `fields` names the
/// settings that changed, never their values.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IntegrationEvent {
    pub id: String,
    pub agent_id: AgentId,
    pub integration_id: Option<String>,
    /// The integration's name at the time, or the family for a grant event.
    pub integration_name: String,
    pub family: IntegrationFamily,
    pub actor: Requester,
    pub action: EventAction,
    pub fields: Vec<String>,
    /// The session an agent acted from.
    pub session: Option<String>,
    pub at: DateTime<Utc>,
}

/// What a write records about itself; storage fills in the rest of the [`IntegrationEvent`].
#[derive(Debug, Clone)]
pub struct EventDraft {
    pub actor: Requester,
    pub action: EventAction,
    pub fields: Vec<String>,
    pub session: Option<String>,
}

impl EventDraft {
    pub fn new(actor: Requester, action: EventAction) -> Self {
        Self { actor, action, fields: vec![], session: None }
    }
}
