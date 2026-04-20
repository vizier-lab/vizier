use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

use crate::schema::AgentId;

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct Memory {
    pub slug: String,
    pub title: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub embedding: Vec<f64>,
    pub agent_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct DocumentIndex {
    pub path: String,
    pub embedding: Vec<f64>,
    pub context: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct Skill {
    pub name: String,
    pub agent_id: Option<AgentId>,
    pub author: String,
    pub description: String,
    pub content: String,
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