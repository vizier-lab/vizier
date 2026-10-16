//! Integrations (specs/016-integrations-section): channel connections, addon configurations and
//! MCP servers, each defined once and attached to agents.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::schema::AgentId;

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

/// Fixed when the integration is created (research R6): `shared` if the creator could manage
/// integrations on every agent, otherwise `owned`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationScope {
    Shared,
    Owned,
}

impl IntegrationScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::Owned => "owned",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "shared" => Self::Shared,
            "owned" => Self::Owned,
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
    pub scope: IntegrationScope,
    pub created_by: Option<String>,
    /// The agents it is attached to, from `integration_attachment`.
    #[serde(default)]
    pub agents: Vec<AgentId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
/// subscribers re-read storage.
#[derive(Debug, Clone)]
pub struct IntegrationChanged {
    pub integration_id: String,
    pub family: IntegrationFamily,
    /// Attached before ∪ attached after the change.
    pub agents: Vec<AgentId>,
}
