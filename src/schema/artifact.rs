//! Permanent artifact domain types (`specs/035-add-artifacts`).
//!
//! Metadata lives in SQLite, content on disk. These are the value types shared by the storage
//! trait, the `Artifacts` service, the HTTP views and the agent tools.

use base64::Engine;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::Requester;

pub type ArtifactId = String;

/// What an artifact is, derived from its MIME type (or extension) at promotion.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Image,
    Pdf,
    Text,
    Markdown,
    Csv,
    Audio,
    Video,
    Other,
}

impl ArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Pdf => "pdf",
            Self::Text => "text",
            Self::Markdown => "markdown",
            Self::Csv => "csv",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Other => "other",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "image" => Self::Image,
            "pdf" => Self::Pdf,
            "text" => Self::Text,
            "markdown" => Self::Markdown,
            "csv" => Self::Csv,
            "audio" => Self::Audio,
            "video" => Self::Video,
            "other" => Self::Other,
            _ => return None,
        })
    }

    /// The kind mapping in `data-model.md` §6. The extension is a fallback for a filename whose
    /// MIME guess is generic (`application/octet-stream`).
    pub fn from_mime(mime: &str, filename: &str) -> Self {
        let mime = mime
            .split(';')
            .next()
            .unwrap_or(mime)
            .trim()
            .to_ascii_lowercase();
        let ext = filename
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .unwrap_or_default();

        if mime.starts_with("image/") {
            return Self::Image;
        }
        if mime.starts_with("audio/") {
            return Self::Audio;
        }
        if mime.starts_with("video/") {
            return Self::Video;
        }
        if mime == "application/pdf" || ext == "pdf" {
            return Self::Pdf;
        }
        if mime == "text/markdown" || ext == "md" || ext == "markdown" {
            return Self::Markdown;
        }
        if mime == "text/csv" || ext == "csv" {
            return Self::Csv;
        }
        if mime.starts_with("text/")
            || matches!(
                mime.as_str(),
                "application/json"
                    | "application/xml"
                    | "application/yaml"
                    | "application/x-yaml"
                    | "application/toml"
            )
        {
            return Self::Text;
        }
        Self::Other
    }
}

impl std::fmt::Display for ArtifactKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A permanent, agent-owned file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct Artifact {
    pub id: String,
    pub agent_id: String,
    pub name: String,
    pub kind: ArtifactKind,
    pub mime_type: String,
    pub size: u64,
    pub version: u32,
    /// The current content file name inside the artifact directory (`content.v<n>`).
    pub content_file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_topic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_filename: Option<String>,
    pub created_by: Requester,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Artifact {
    pub fn origin(&self) -> ArtifactOrigin {
        ArtifactOrigin {
            channel: self.origin_channel.clone(),
            topic: self.origin_topic.clone(),
            source_filename: self.source_filename.clone(),
        }
    }
}

/// Where an artifact came from: the promoting conversation and the session file that supplied
/// the current content. All parts are optional (an agent promotion in a dream has a source
/// conversation; a migration may have none).
#[derive(
    Debug, Clone, Default, Serialize, Deserialize, JsonSchema, utoipa::ToSchema, PartialEq,
)]
pub struct ArtifactOrigin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_filename: Option<String>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactEventAction {
    Promoted,
    Replaced,
    Deleted,
}

impl ArtifactEventAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Promoted => "promoted",
            Self::Replaced => "replaced",
            Self::Deleted => "deleted",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "promoted" => Self::Promoted,
            "replaced" => Self::Replaced,
            "deleted" => Self::Deleted,
            _ => return None,
        })
    }
}

/// One successful promotion, replacement or deletion. Never stores content, and survives the
/// artifact row (agent deletion removes it).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct ArtifactEvent {
    pub id: String,
    pub agent_id: String,
    pub artifact_id: String,
    pub artifact_name: String,
    pub action: ArtifactEventAction,
    pub actor: Requester,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_topic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_filename: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A derived index row of where an artifact is used (`message` or `memory`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct ArtifactReference {
    pub artifact_id: String,
    pub agent_id: String,
    /// `message` or `memory`.
    pub kind: String,
    /// `message:<session_slug>:<history_uid>` or `memory:<bundle>:<path>`.
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub label: String,
    pub created_at: DateTime<Utc>,
}

impl ArtifactReference {
    pub fn message_target(session_slug: &str, history_uid: &str) -> String {
        format!("message:{session_slug}:{history_uid}")
    }

    pub fn memory_target(bundle: &str, path: &str) -> String {
        format!("memory:{bundle}:{path}")
    }
}

/// One keyset page of artifacts, newest `updated_at` first.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct ArtifactPage {
    pub items: Vec<Artifact>,
    /// The last item's cursor, or `None` on the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    /// Items matching the filters, before the cursor.
    pub total: usize,
    pub total_bytes: u64,
    /// The agent's whole collection, ignoring filters.
    pub agent_total: usize,
    pub agent_total_bytes: u64,
}

/// What a delete did. Message references remain (the history row keeps the artifact id and
/// reads as unavailable); memory links are removed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct ArtifactDeleteResult {
    pub removed: bool,
    pub references: usize,
    pub memory_links_removed: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct ArtifactStats {
    pub total: usize,
    pub total_bytes: u64,
}

/// The gallery's query. `from`/`to` are inclusive bounds on `updated_at`; `kind` and `q` are
/// optional filters; `cursor`/`limit` page.
#[derive(Debug, Clone, Default)]
pub struct ArtifactFilters {
    pub agent_id: String,
    pub q: Option<String>,
    pub kind: Option<ArtifactKind>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub cursor: Option<ArtifactCursor>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactCursor {
    pub at_ms: i64,
    pub id: String,
}

impl ArtifactCursor {
    pub fn new(at: DateTime<Utc>, id: &str) -> Self {
        Self {
            at_ms: at.timestamp_millis(),
            id: id.to_string(),
        }
    }

    pub fn encode(&self) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(format!("{}:{}", self.at_ms, self.id))
    }

    pub fn decode(raw: &str) -> Option<Self> {
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(raw)
            .ok()?;
        let text = String::from_utf8(bytes).ok()?;
        let (at, id) = text.split_once(':')?;
        Some(Self {
            at_ms: at.parse().ok()?,
            id: id.to_string(),
        })
    }
}

/// Validate and normalise a display name: trimmed, 1–255 characters, no path separators, no
/// ASCII control characters, and not `.`/`..`. The display name is metadata only and is never
/// used as a filesystem path.
pub fn validate_artifact_name(name: &str) -> crate::Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(crate::error::VizierError(
            "artifact name must not be empty".into(),
        ));
    }
    if trimmed.chars().count() > 255 {
        return Err(crate::error::VizierError(
            "artifact name must be at most 255 characters".into(),
        ));
    }
    if trimmed == "." || trimmed == ".." {
        return Err(crate::error::VizierError(
            "artifact name must not be '.' or '..'".into(),
        ));
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err(crate::error::VizierError(
            "artifact name must not contain '/' or '\\'".into(),
        ));
    }
    if trimmed.chars().any(|c| c.is_ascii_control()) {
        return Err(crate::error::VizierError(
            "artifact name must not contain control characters".into(),
        ));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(name: &str) -> Artifact {
        Artifact {
            id: "id".into(),
            agent_id: "a".into(),
            name: name.into(),
            kind: ArtifactKind::Text,
            mime_type: "text/plain".into(),
            size: 1,
            version: 1,
            content_file: "content.v1".into(),
            origin_channel: None,
            origin_topic: None,
            source_filename: None,
            created_by: Requester::Agent("a".into()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn artifacts_round_trip_through_json() {
        let a = artifact("Q3-report.pdf");
        let json = serde_json::to_string(&a).unwrap();
        let back: Artifact = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, a.name);
        assert_eq!(back.kind, a.kind);
        assert_eq!(back.version, 1);
    }

    #[test]
    fn names_are_case_sensitive_and_duplicates_are_a_storage_concern() {
        assert_ne!(artifact("Report.pdf").name, artifact("report.pdf").name);
    }

    #[test]
    fn names_are_trimmed_and_validated() {
        assert_eq!(
            validate_artifact_name("  report.pdf  ").unwrap(),
            "report.pdf"
        );
        for bad in ["", "   ", ".", "..", "a/b", "a\\b", "a\u{0000}b"] {
            assert!(validate_artifact_name(bad).is_err(), "{bad:?} was accepted");
        }
        assert!(validate_artifact_name(&"x".repeat(255)).is_ok());
        assert!(validate_artifact_name(&"x".repeat(256)).is_err());
        assert!(validate_artifact_name("Résumé – 2026.csv").is_ok());
    }

    #[test]
    fn kinds_follow_the_mime_and_extension_table() {
        let k = |mime, name| ArtifactKind::from_mime(mime, name);
        assert_eq!(k("image/png", "x.png"), ArtifactKind::Image);
        assert_eq!(k("application/pdf", "x.bin"), ArtifactKind::Pdf);
        assert_eq!(k("application/octet-stream", "x.pdf"), ArtifactKind::Pdf);
        assert_eq!(k("text/markdown", "x"), ArtifactKind::Markdown);
        assert_eq!(
            k("application/octet-stream", "x.md"),
            ArtifactKind::Markdown
        );
        assert_eq!(k("text/csv", "x"), ArtifactKind::Csv);
        assert_eq!(k("application/octet-stream", "x.csv"), ArtifactKind::Csv);
        assert_eq!(k("text/plain", "x.txt"), ArtifactKind::Text);
        assert_eq!(k("application/json", "x.json"), ArtifactKind::Text);
        assert_eq!(k("application/xml", "x.xml"), ArtifactKind::Text);
        assert_eq!(k("application/yaml", "x.yaml"), ArtifactKind::Text);
        assert_eq!(k("application/x-yaml", "x.yml"), ArtifactKind::Text);
        assert_eq!(k("application/toml", "x.toml"), ArtifactKind::Text);
        assert_eq!(k("audio/mpeg", "x.mp3"), ArtifactKind::Audio);
        assert_eq!(k("video/mp4", "x.mp4"), ArtifactKind::Video);
        assert_eq!(k("application/zip", "x.zip"), ArtifactKind::Other);
    }

    #[test]
    fn kind_and_action_strings_round_trip() {
        for kind in [
            ArtifactKind::Image,
            ArtifactKind::Pdf,
            ArtifactKind::Text,
            ArtifactKind::Markdown,
            ArtifactKind::Csv,
            ArtifactKind::Audio,
            ArtifactKind::Video,
            ArtifactKind::Other,
        ] {
            assert_eq!(ArtifactKind::parse(kind.as_str()), Some(kind));
        }
        for action in [
            ArtifactEventAction::Promoted,
            ArtifactEventAction::Replaced,
            ArtifactEventAction::Deleted,
        ] {
            assert_eq!(ArtifactEventAction::parse(action.as_str()), Some(action));
        }
    }

    #[test]
    fn cursors_round_trip_and_reject_garbage() {
        let at = DateTime::parse_from_rfc3339("2026-10-29T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let cursor = ArtifactCursor::new(at, "abc");
        assert_eq!(ArtifactCursor::decode(&cursor.encode()), Some(cursor));
        assert_eq!(ArtifactCursor::decode("not-a-cursor"), None);
    }
}
