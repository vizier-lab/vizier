//! Usage accounting (`specs/018-usage-accounting/`): one recorded event per answered model call,
//! and the report the usage page is built from.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Local};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::metrics::TokenCounts;

/// What a model call was for. Decided once, when the call is recorded (research R4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum UsageActivity {
    Chat,
    /// A project chat's coding work (`specs/041-project-coding-workspace` FR-016).
    Project,
    /// A Workspace topic (`specs/053-workspace-channel`). Workspaces were removed by spec 064;
    /// kept so usage recorded before still reads (#82).
    Workspace,
    Background,
    Scheduled,
    Dreams,
    Compaction,
    Vision,
    Other,
    /// A turn of a consulted agent (`specs/064-consult-agents`).
    Consultation,
    /// A media call made outside any conversation to keep an agent's memory or skills indexed
    /// (`specs/065-media-usage-accounting` FR-003).
    Indexing,
}

impl UsageActivity {
    pub const ALL: [UsageActivity; 11] = [
        Self::Chat,
        Self::Project,
        Self::Workspace,
        Self::Background,
        Self::Scheduled,
        Self::Dreams,
        Self::Compaction,
        Self::Vision,
        Self::Other,
        Self::Consultation,
        Self::Indexing,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Project => "project",
            Self::Workspace => "workspace",
            Self::Background => "background",
            Self::Scheduled => "scheduled",
            Self::Dreams => "dreams",
            Self::Compaction => "compaction",
            Self::Vision => "vision",
            Self::Other => "other",
            Self::Consultation => "consultation",
            Self::Indexing => "indexing",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Chat => "Chat",
            Self::Project => "Projects",
            Self::Workspace => "Workspaces (removed)",
            Self::Background => "Background",
            Self::Scheduled => "Scheduled",
            Self::Dreams => "Dreams",
            Self::Compaction => "Compaction",
            Self::Vision => "Vision",
            Self::Other => "Other",
            Self::Consultation => "Consultations",
            Self::Indexing => "Indexing",
        }
    }

    /// Activities that were not recorded before complete accounting started (FR-019).
    pub fn missing_before_accounting(&self) -> bool {
        matches!(self, Self::Dreams | Self::Compaction | Self::Vision)
    }
}

impl std::str::FromStr for UsageActivity {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL.into_iter().find(|a| a.as_str() == s).ok_or(())
    }
}

/// What kind of model call a usage record is (`specs/065-media-usage-accounting` data-model §1).
/// Every completion is `Text`. Display order; append only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum UsageKind {
    #[default]
    Text,
    Image,
    Speech,
    Transcription,
    Embedding,
}

impl UsageKind {
    pub const ALL: [UsageKind; 5] = [
        Self::Text,
        Self::Image,
        Self::Speech,
        Self::Transcription,
        Self::Embedding,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Speech => "speech",
            Self::Transcription => "transcription",
            Self::Embedding => "embedding",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Image => "Images",
            Self::Speech => "Speech",
            Self::Transcription => "Transcription",
            Self::Embedding => "Embeddings",
        }
    }
}

impl std::str::FromStr for UsageKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL.into_iter().find(|k| k.as_str() == s).ok_or(())
    }
}

/// A media call's amount in its kind's own unit (data-model §3). Zero is "not this unit" or, for
/// `audio_ms`, unknown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MediaUnits {
    pub images: u64,
    pub characters: u64,
    pub audio_ms: u64,
}

impl std::ops::AddAssign for MediaUnits {
    fn add_assign(&mut self, rhs: Self) {
        self.images += rhs.images;
        self.characters += rhs.characters;
        self.audio_ms += rhs.audio_ms;
    }
}

/// One answered model call, as stored in `usage_event`.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageEvent {
    pub agent_id: String,
    pub at: DateTime<Local>,
    pub activity: UsageActivity,
    /// `VizierChannelId::to_slug()`; `None` for a call with no session.
    pub channel: Option<String>,
    pub topic: Option<String>,
    /// The Channel-breakdown key, `VizierSession::usage_place()`.
    pub place: String,
    pub turn_id: Option<String>,
    pub round: Option<u32>,
    /// Empty only on carried-over rows that never recorded one.
    pub provider: String,
    pub model: String,
    pub served_model: Option<String>,
    pub tokens: TokenCounts,
    pub duration_ms: u64,
    pub legacy: bool,
    /// `Text` for completions (spec 065).
    pub kind: UsageKind,
    /// Image only: `<size>` or `<size>/<quality>`.
    pub variant: Option<String>,
    pub units: MediaUnits,
    /// Some amount was measured rather than reported by the provider (research R4).
    pub measured: bool,
    /// The model runs on this machine; priced at zero (FR-010).
    pub local: bool,
    /// The `execute_python` call that made it (research R10).
    pub call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UsageRange {
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct FailedTotals {
    /// `null` when nothing failed is priced.
    pub cost_usd: Option<f64>,
    pub tokens: u64,
    pub turns: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UsageSummary {
    /// `null` (not `0`) when nothing in scope has a known price.
    pub cost_usd: Option<f64>,
    pub tokens: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub output_tokens: u64,
    pub calls: u64,
    pub cache_hit_rate: Option<f64>,
    pub failed: FailedTotals,
    /// Set only when the report is narrowed to one activity: its share of all activities' cost.
    pub share_of_all: Option<f64>,
    /// Set only when the report is narrowed to one kind (spec 065): its share of all kinds' cost
    /// under the same activity filter.
    pub kind_share_of_all: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PeriodTotals {
    pub cost_usd: Option<f64>,
    pub tokens: u64,
    pub calls: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DayActivity {
    pub cost_usd: Option<f64>,
    pub tokens: u64,
    pub calls: u64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DailyUsage {
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub date: NaiveDate,
    /// Keyed by `UsageActivity` (snake_case).
    pub by_activity: BTreeMap<UsageActivity, DayActivity>,
    /// Keyed by `UsageKind` (snake_case), for the day's tooltip (spec 065).
    pub by_kind: BTreeMap<UsageKind, DayActivity>,
}

/// A media amount in its kind's own unit (spec 065 data-model §9). Only the fields of the kinds
/// in the row are set.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Units {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub characters: Option<u64>,
    /// Known durations only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_seconds: Option<f64>,
    /// Embedding input tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
}

impl Units {
    /// The amount of `kind` in `units`/`tokens`, or `None` for text.
    pub fn of(kind: UsageKind, units: &MediaUnits, tokens: u64) -> Option<Self> {
        let seconds = (units.audio_ms > 0).then(|| units.audio_ms as f64 / 1000.0);
        match kind {
            UsageKind::Text => None,
            UsageKind::Image => Some(Self {
                images: Some(units.images),
                ..Default::default()
            }),
            UsageKind::Speech => Some(Self {
                characters: Some(units.characters),
                audio_seconds: seconds,
                ..Default::default()
            }),
            UsageKind::Transcription => Some(Self {
                audio_seconds: Some(units.audio_ms as f64 / 1000.0),
                ..Default::default()
            }),
            UsageKind::Embedding => Some(Self {
                tokens: Some(tokens),
                ..Default::default()
            }),
        }
    }

    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

// Another source that prices a model row differently (spec 068 FR-006), priced on the row's own
// tokens and units. A plain comment: a doc comment would become the schema's description.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PriceAlternative {
    pub source: String,
    pub cost_usd: f64,
}

// A catalogue source a report's labels name, with its display name (spec 068).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SourceSummary {
    pub id: String,
    pub name: String,
    pub fetched_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BreakdownRow {
    pub key: String,
    pub label: String,
    pub detail: Option<String>,
    pub cost_usd: Option<f64>,
    /// Fraction of the report's cost; `null` for an unpriced row.
    pub share: Option<f64>,
    pub tokens: u64,
    pub calls: u64,
    /// The row's media amounts, when it has media calls (spec 065).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub units: Option<Units>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct KindRow {
    #[serde(flatten)]
    pub row: BreakdownRow,
    pub kind: UsageKind,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ModelRow {
    #[serde(flatten)]
    pub row: BreakdownRow,
    pub provider: String,
    pub model: String,
    pub served_models: Vec<String>,
    pub cache_hit_rate: Option<f64>,
    pub failed_tokens: u64,
    pub priced: bool,
    pub kind: UsageKind,
    /// Which source or rule priced the row (spec 068): `models_dev`, `openrouter`, `venice`,
    /// `curated`, `inference_profile`, `local`, `subscription`, `borrowed`, or `none`. Open: a
    /// new source adds a value, never a schema change.
    pub price_source: String,
    /// The provider a `borrowed` price came from.
    pub borrowed_from: Option<String>,
    /// Up to three other sources that price this row differently.
    pub price_alternatives: Vec<PriceAlternative>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ConversationRow {
    pub channel: String,
    pub topic: Option<String>,
    pub label: String,
    pub detail: Option<String>,
    /// The activity, and the model, that cost the most in this conversation.
    pub activity: UsageActivity,
    pub model: String,
    pub tokens: u64,
    pub cost_usd: Option<f64>,
    pub failed_share: Option<f64>,
    pub last_at: DateTime<Local>,
    pub exists: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UnpricedNote {
    pub tokens: u64,
    pub models: Vec<String>,
    /// Unpriced media amounts, by kind (spec 065 FR-011).
    pub units: BTreeMap<UsageKind, Units>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UsageReport {
    pub range: UsageRange,
    /// When complete accounting started (R6); ranges before it lack dreams, compaction and
    /// failed rounds.
    pub accounting_since: Option<DateTime<Local>>,
    pub activity: Option<UsageActivity>,
    pub kind: Option<UsageKind>,
    /// When media calls started being recorded (spec 065 R8).
    pub media_accounting_since: Option<DateTime<Local>>,
    pub summary: UsageSummary,
    pub previous: Option<PeriodTotals>,
    pub daily: Vec<DailyUsage>,
    pub by_activity: Vec<BreakdownRow>,
    /// Empty when the report is narrowed to one kind.
    pub by_kind: Vec<KindRow>,
    pub by_model: Vec<ModelRow>,
    pub by_place: Vec<BreakdownRow>,
    pub costliest: Vec<ConversationRow>,
    pub unpriced: UnpricedNote,
    /// The catalogue sources the rows' labels name, for their display names (spec 068).
    pub sources: Vec<SourceSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CallRow {
    pub at: DateTime<Local>,
    pub activity: UsageActivity,
    pub round: Option<u32>,
    pub provider: String,
    pub model: String,
    pub served_model: Option<String>,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: Option<f64>,
    pub failed: bool,
    pub kind: UsageKind,
    pub variant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub units: Option<Units>,
    pub measured: bool,
}

/// One media record made by a tool call, priced (spec 065 R10, `GET …/usage/calls`).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct MediaCall {
    pub kind: UsageKind,
    pub provider: String,
    pub model: String,
    pub variant: Option<String>,
    pub units: Option<Units>,
    pub measured: bool,
    pub local: bool,
    pub cost_usd: Option<f64>,
    /// Which source or rule priced the call, as on a model row.
    pub price_source: String,
    pub at: DateTime<Local>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct MediaCalls {
    /// Keyed by `execute_python` call id; an id with no media records is absent.
    pub calls: BTreeMap<String, Vec<MediaCall>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum UsageLinkKind {
    Chat,
    TaskRun,
    Dream,
    Topic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UsageLink {
    pub kind: UsageLinkKind,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum NoLinkReason {
    NoWebPage,
    Deleted,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ConversationUsage {
    pub label: String,
    pub detail: Option<String>,
    pub summary: UsageSummary,
    pub insight: Option<String>,
    pub calls: Vec<CallRow>,
    pub more: u64,
    pub link: Option<UsageLink>,
    pub no_link_reason: Option<NoLinkReason>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_round_trips_through_its_name() {
        for k in UsageKind::ALL {
            assert_eq!(k.as_str().parse::<UsageKind>(), Ok(k));
            assert_eq!(serde_json::to_value(k).unwrap(), k.as_str());
        }
    }

    #[test]
    fn activity_round_trips_through_its_name() {
        for a in UsageActivity::ALL {
            assert_eq!(a.as_str().parse::<UsageActivity>(), Ok(a));
            assert_eq!(serde_json::to_value(a).unwrap(), a.as_str());
        }
        assert!("nope".parse::<UsageActivity>().is_err());
    }
}
