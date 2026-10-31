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
    pub attachments: Vec<MemoryAttachment>,
    /// Mirrors `attachments.len()` on a full read; cached separately so listing paths served
    /// from the Memory Graph Index (which never carry the full `attachments` payload) can still
    /// report how many attachments a concept has.
    #[serde(default)]
    pub attachment_count: usize,
    #[serde(default)]
    pub read_count: u64,
}

/// An attachment on a memory concept document (`specs/035-add-artifacts`).
///
/// New writes only produce [`MemoryAttachment::Artifact`]. The pre-035 shape
/// `{ filename, content: { local } }` still parses as [`MemoryAttachment::LegacySessionFile`]
/// and is read-only; it is not migrated and is tracked for removal in #82.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum MemoryAttachment {
    Artifact { artifact_id: String, name: String },
    LegacySessionFile(VizierAttachment),
}

/// Read by key rather than as an untagged enum: a legacy attachment's `content` is itself an
/// enum, which serde's untagged buffering cannot re-deserialize. The document stays the two
/// shapes above; only the reader is explicit.
impl<'de> Deserialize<'de> for MemoryAttachment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct ArtifactAttachment {
            artifact_id: String,
            name: String,
        }

        // YAML represents an externally tagged enum (the legacy attachment's `content`) as a
        // tag, so an untagged enum cannot re-read it. Capture the YAML value and dispatch by
        // key instead; `serde_yaml::from_value` keeps the tags intact.
        let value = serde_yaml::Value::deserialize(deserializer)?;
        let is_artifact = value
            .as_mapping()
            .map(|map| {
                map.contains_key(serde_yaml::Value::String("artifact_id".into()))
                    && map.contains_key(serde_yaml::Value::String("name".into()))
            })
            .unwrap_or(false);
        if is_artifact {
            let artifact = serde_yaml::from_value::<ArtifactAttachment>(value)
                .map_err(serde::de::Error::custom)?;
            return Ok(Self::Artifact {
                artifact_id: artifact.artifact_id,
                name: artifact.name,
            });
        }
        serde_yaml::from_value::<VizierAttachment>(value)
            .map(Self::LegacySessionFile)
            .map_err(serde::de::Error::custom)
    }
}

impl MemoryAttachment {
    /// The display name: the artifact name, or the legacy filename.
    pub fn display_name(&self) -> &str {
        match self {
            Self::Artifact { name, .. } => name,
            Self::LegacySessionFile(attachment) => &attachment.filename,
        }
    }

    pub fn artifact_id(&self) -> Option<&str> {
        match self {
            Self::Artifact { artifact_id, .. } => Some(artifact_id),
            Self::LegacySessionFile(_) => None,
        }
    }
}

/// The HTTP view of a memory attachment: tagged by kind and carrying read-time availability.
/// The stored document stays the untagged [`MemoryAttachment`] enum.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MemoryAttachmentView {
    Artifact {
        artifact_id: String,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<u64>,
        available: bool,
    },
    LegacySessionFile {
        filename: String,
        content: crate::schema::VizierAttachmentContent,
        available: bool,
    },
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BundleSummary {
    pub name: String,
    pub concept_count: usize,
    pub updated_at: Option<DateTime<Utc>>,
    /// Links from this bundle's documents (to anywhere), for the "Most linked" sort.
    #[serde(default)]
    pub link_count: usize,
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
