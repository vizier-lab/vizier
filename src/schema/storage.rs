use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

use crate::schema::AgentId;

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, SurrealValue)]
pub enum MemoryVisibility {
    #[serde(rename = "private")]
    #[default]
    Private,
    #[serde(rename = "global")]
    Global,
    #[serde(rename = "shared")]
    Shared,
}

impl std::fmt::Display for MemoryVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Private => write!(f, "private"),
            Self::Global => write!(f, "global"),
            Self::Shared => write!(f, "shared"),
        }
    }
}

impl std::str::FromStr for MemoryVisibility {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "private" => Ok(Self::Private),
            "global" => Ok(Self::Global),
            "shared" => Ok(Self::Shared),
            _ => Err(format!("invalid visibility: {s}")),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct Memory {
    pub slug: String,
    pub title: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub embedding: Vec<f64>,
    pub agent_id: String,
    #[serde(default)]
    pub visibility: MemoryVisibility,
    #[serde(default)]
    pub shared_to: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct DocumentIndex {
    pub path: String,
    pub embedding: Vec<f64>,
    pub context: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, schemars::JsonSchema, SurrealValue)]
pub enum SkillActivation {
    #[serde(rename = "always")]
    Always,
    #[serde(rename = "on_demand")]
    OnDemand,
    #[serde(rename = "contextual")]
    Contextual,
}

impl Default for SkillActivation {
    fn default() -> Self {
        Self::OnDemand
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct Skill {
    pub name: String,
    pub agent_id: Option<AgentId>,
    pub author: String,
    pub description: String,
    pub content: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default = "default_activation")]
    pub activation: SkillActivation,
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub resources: Vec<String>,
}

fn default_activation() -> SkillActivation {
    SkillActivation::OnDemand
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct SharedDocument {
    pub slug: String,
    pub title: String,
    pub content: String,
    pub author_agent_id: AgentId,
    pub timestamp: DateTime<Utc>,
    pub embedding: Vec<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct SharedDocumentSummary {
    pub slug: String,
    pub title: String,
    pub author_agent_id: AgentId,
    pub timestamp: DateTime<Utc>,
}