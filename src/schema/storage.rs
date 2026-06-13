use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

use crate::{
    schema::{AgentId, VizierAttachment},
    utils::markdown::MarkdownDoc,
};

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, SurrealValue, utoipa::ToSchema)]
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

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue, MarkdownDoc)]
pub struct Memory {
    pub slug: String,
    pub title: String,
    #[markdown(content)]
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub agent_id: String,
    #[serde(default)]
    pub visibility: MemoryVisibility,
    #[serde(default)]
    pub shared_to: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub attachments: Vec<VizierAttachment>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryQueryParams {
    pub agent_id: String,
    pub tags: Option<Vec<String>>,
    pub visibility: Option<MemoryVisibility>,
    pub offset: usize,
    pub limit: usize,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaginatedMemory {
    pub memories: Vec<Memory>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema)]
pub struct MemoryGraphNode {
    pub slug: String,
    pub title: String,
    pub tags: Vec<String>,
    pub visibility: MemoryVisibility,
    pub agent_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema)]
pub struct MemoryGraphEdge {
    pub source: String,
    pub target: String,
    pub broken: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema)]
pub struct MemoryGraph {
    pub nodes: Vec<MemoryGraphNode>,
    pub edges: Vec<MemoryGraphEdge>,
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

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue, MarkdownDoc)]
pub struct Skill {
    pub name: String,
    pub agent_id: Option<AgentId>,
    pub author: String,
    pub description: String,
    #[markdown(content)]
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