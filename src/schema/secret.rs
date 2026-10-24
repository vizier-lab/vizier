//! An agent's secret change log (`specs/027-agent-secrets/data-model.md` §2–3): who created,
//! replaced or deleted which secret name, and when — never a value.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::{AgentId, Requester};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SecretAction {
    Created,
    Replaced,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct AgentSecretEvent {
    pub id: String,
    pub agent_id: AgentId,
    pub name: String,
    pub action: SecretAction,
    pub actor: Requester,
    pub at: DateTime<Utc>,
}
