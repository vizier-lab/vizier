use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::{
    schema::AgentId,
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
    pub created_at: DateTime<Local>,
    /// Set on every write.
    pub updated_at: DateTime<Local>,
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
    pub read_count: u64,
}

pub const DEFAULT_BUNDLE: &str = "default";

/// The reserved bundle every agent has exactly one of, holding the identity document
/// (specs/043-unify-core-and-memory). It cannot be removed, and no other document may be
/// written into it. `core` is a reserved *address*, not a product term: the product-facing
/// name for the identity stays **Core**, and the bundle/path here are internal.
pub const CORE_BUNDLE: &str = "core";

/// The identity document's path inside [`CORE_BUNDLE`]. The path segment is an internal
/// detail the agent addresses; it is not renamed on any product surface.
pub const SOUL_PATH: &str = "soul";

pub fn default_bundle() -> String {
    DEFAULT_BUNDLE.to_string()
}

/// True when `bundle` is the reserved identity bundle.
pub fn is_core_bundle(bundle: &str) -> bool {
    bundle == CORE_BUNDLE
}

/// True when `(bundle, path)` is the identity document's address.
pub fn is_identity_address(bundle: &str, path: &str) -> bool {
    is_core_bundle(bundle) && path == SOUL_PATH
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BundleSummary {
    pub name: String,
    pub concept_count: usize,
    pub updated_at: Option<DateTime<Local>>,
    /// Links from this bundle's documents (to anywhere), for the "Most linked" sort.
    #[serde(default)]
    pub link_count: usize,
    /// The bundle cannot be deleted, though the documents inside it can
    /// (specs/043-unify-core-and-memory). Additive: a client that ignores it is unchanged.
    #[serde(default)]
    pub protected: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ImportReport {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct MemoryGraphNode {
    pub slug: String,
    pub bundle: String,
    pub title: String,
    pub tags: Vec<String>,
    pub agent_id: String,
    #[serde(default)]
    pub boundary: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct MemoryGraphEdge {
    pub source: String,
    pub target: String,
    pub broken: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
#[derive(Debug, Serialize, Deserialize, Clone, schemars::JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
    /// Who wrote it. Optional: the Agent Skills ecosystem does not require the field, and none of
    /// the spec-kit or impeccable skills carry it (`specs/041-project-coding-workspace` FR-038).
    #[serde(default)]
    pub author: String,
    pub description: String,
    #[markdown(content)]
    pub content: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default = "default_version")]
    pub version: SkillVersion,
    #[serde(default)]
    pub resources: Vec<String>,
    /// Whether a person may run this skill by name (`/<slug>`). Absent means yes, which is the
    /// Agent Skills default and what every workspace skill relied on before the field existed.
    #[serde(rename = "user-invocable", default = "default_true")]
    pub user_invocable: bool,
    /// Set when the skill must not be offered to the model: it is prose for a person to run, not
    /// something the agent should pick up on its own.
    #[serde(rename = "disable-model-invocation", default)]
    pub disable_model_invocation: bool,
    /// A short usage line the person sees, e.g. `[shape · audit] [target]`.
    #[serde(rename = "argument-hint", default, skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,
}

fn default_version() -> SkillVersion {
    SkillVersion::Number(1)
}

fn default_true() -> bool {
    true
}

/// A skill's `version`, which the ecosystem writes both ways: Vizier and spec-kit write a number,
/// impeccable writes `4.7.0`. Whichever was read is written back, so a save never rewrites a
/// project's file into a shape it did not have.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum SkillVersion {
    Number(u32),
    Text(String),
}

impl Default for SkillVersion {
    fn default() -> Self {
        Self::Number(1)
    }
}

impl SkillVersion {
    /// The leading number, for the places that only track "was this installed": `4.7.0` is 4,
    /// `12` is 12, and anything unparsable is 1. `SkillMeta::installed_version` is written but
    /// never compared, so nothing depends on it being exact.
    pub fn leading_number(&self) -> u32 {
        match self {
            Self::Number(n) => *n,
            Self::Text(text) => text
                .split(|c: char| !c.is_ascii_digit())
                .find(|part| !part.is_empty())
                .and_then(|part| part.parse().ok())
                .unwrap_or(1),
        }
    }

    /// The next version after an edit. A numeric version counts up; a text version (`4.7.0`) is
    /// left as it is, because "next" has no meaning in a shape Vizier did not write.
    pub fn bumped(&self) -> Self {
        match self {
            Self::Number(n) => Self::Number(n.saturating_add(1)),
            Self::Text(_) => self.clone(),
        }
    }
}

impl std::fmt::Display for SkillVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::Text(text) => write!(f, "{text}"),
        }
    }
}
