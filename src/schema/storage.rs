use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};


use crate::{
    schema::{AgentId, VizierAttachment},
    utils::markdown::MarkdownDoc,
};

#[derive(Debug, Serialize, Deserialize, Clone, MarkdownDoc)]
pub struct Memory {
    /// Relative path of this concept within its bundle, without extension
    /// (e.g. "friends/bred" for a nested concept, or "project-architecture" at the bundle root).
    /// This doubles as the concept's addressable identity alongside `bundle`.
    pub slug: String,
    pub title: String,
    #[markdown(content)]
    pub content: String,
    /// Set once, on first write.
    pub created_at: DateTime<Utc>,
    /// Set on every write.
    pub updated_at: DateTime<Utc>,
    pub agent_id: String,
    /// Name of the bundle this concept belongs to.
    #[serde(default = "default_bundle")]
    pub bundle: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub attachments: Vec<VizierAttachment>,
    /// Mirrors `attachments.len()` on a full read; cached separately so listing paths served
    /// from the Memory Graph Index (which never carry the full `attachments` payload) can still
    /// report how many attachments a concept has.
    #[serde(default)]
    pub attachment_count: usize,
    #[serde(default)]
    pub read_count: u64,
}

pub const DEFAULT_BUNDLE: &str = "default";

pub fn default_bundle() -> String {
    DEFAULT_BUNDLE.to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryQueryParams {
    pub agent_id: String,
    #[serde(default)]
    pub bundle: Option<String>,
    pub tags: Option<Vec<String>>,
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
pub struct BundleSummary {
    pub name: String,
    pub concept_count: usize,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, utoipa::ToSchema)]
pub struct ImportReport {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema)]
pub struct MemoryGraphNode {
    pub slug: String,
    pub bundle: String,
    pub title: String,
    pub tags: Vec<String>,
    pub agent_id: String,
    #[serde(default)]
    pub boundary: bool,
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
    pub initial_slugs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocumentIndex {
    pub path: String,
    pub embedding: Vec<f64>,
    pub context: String,
    /// Cosine similarity of this hit against the query, `1.0 - distance`. Required by FR-010 so
    /// results can be ranked on how well they actually matched; the value was already being
    /// computed for the threshold filter and discarded (research Decision 4). `NoopIndexer`
    /// returns `0.0`, which is never read because it returns no results at all.
    #[serde(default)]
    pub score: f64,
}

/// One search hit: a passage of a memory document, carrying everything needed to act on it
/// without searching again (FR-009). Shared by the agent-facing `memory_search` tool, the
/// automatic-context assembly, and the HTTP query endpoint, so one shape serves all three
/// surfaces rather than three near-identical ones (Principle II).
///
/// `text` is sliced out of the document at query time and never stored (research Decision 3).
#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema, schemars::JsonSchema)]
pub struct MemoryPassageResult {
    pub bundle: String,
    pub path: String,
    pub title: String,
    /// 0-based ordinal of this passage within its document; the first ordinal when merged.
    pub ordinal: usize,
    /// Last ordinal when adjacent passages were merged (FR-011); equals `ordinal` otherwise.
    pub ordinal_end: usize,
    /// 1-based line span within the document (FR-007).
    pub line_start: usize,
    pub line_end: usize,
    pub text: String,
    /// Relevance of the passage itself, independent of its document (FR-010).
    pub score: f64,
    /// Set when the text was cut to fit the automatic-context size budget (FR-028). Always
    /// `false` for search results, which are not size-capped.
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, MarkdownDoc)]
pub struct Skill {
    pub name: String,
    pub agent_id: Option<AgentId>,
    pub author: String,
    pub description: String,
    #[markdown(content)]
    pub content: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub resources: Vec<String>,
}

fn default_version() -> u32 {
    1
}