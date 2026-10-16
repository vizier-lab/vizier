//! Usage accounting (`specs/018-usage-accounting/`): one recorded event per answered model call,
//! and the report the usage page is built from.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::metrics::TokenCounts;

/// What a model call was for. Decided once, when the call is recorded (research R4).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    JsonSchema,
    utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum UsageActivity {
    Chat,
    Background,
    Scheduled,
    Dreams,
    Compaction,
    Vision,
    Other,
}

impl UsageActivity {
    pub const ALL: [UsageActivity; 7] = [
        Self::Chat,
        Self::Background,
        Self::Scheduled,
        Self::Dreams,
        Self::Compaction,
        Self::Vision,
        Self::Other,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Background => "background",
            Self::Scheduled => "scheduled",
            Self::Dreams => "dreams",
            Self::Compaction => "compaction",
            Self::Vision => "vision",
            Self::Other => "other",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Chat => "Chat",
            Self::Background => "Background",
            Self::Scheduled => "Scheduled",
            Self::Dreams => "Dreams",
            Self::Compaction => "Compaction",
            Self::Vision => "Vision",
            Self::Other => "Other",
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

/// One answered model call, as stored in `usage_event`.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageEvent {
    pub agent_id: String,
    pub at: DateTime<Utc>,
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
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct UsageRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct FailedTotals {
    /// `null` when nothing failed is priced.
    pub cost_usd: Option<f64>,
    pub tokens: u64,
    pub turns: u64,
}

#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
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
}

#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct PeriodTotals {
    pub cost_usd: Option<f64>,
    pub tokens: u64,
    pub calls: u64,
}

#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct DayActivity {
    pub cost_usd: Option<f64>,
    pub tokens: u64,
    pub calls: u64,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct DailyUsage {
    #[schema(value_type = String)]
    pub date: NaiveDate,
    /// Keyed by `UsageActivity` (snake_case).
    pub by_activity: BTreeMap<UsageActivity, DayActivity>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct BreakdownRow {
    pub key: String,
    pub label: String,
    pub detail: Option<String>,
    pub cost_usd: Option<f64>,
    /// Fraction of the report's cost; `null` for an unpriced row.
    pub share: Option<f64>,
    pub tokens: u64,
    pub calls: u64,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ModelRow {
    #[serde(flatten)]
    pub row: BreakdownRow,
    pub provider: String,
    pub model: String,
    pub served_models: Vec<String>,
    pub cache_hit_rate: Option<f64>,
    pub failed_tokens: u64,
    pub priced: bool,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
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
    pub last_at: DateTime<Utc>,
    pub exists: bool,
}

#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct UnpricedNote {
    pub tokens: u64,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct UsageReport {
    pub range: UsageRange,
    /// When complete accounting started (R6); ranges before it lack dreams, compaction and
    /// failed rounds.
    pub accounting_since: Option<DateTime<Utc>>,
    pub activity: Option<UsageActivity>,
    pub summary: UsageSummary,
    pub previous: Option<PeriodTotals>,
    pub daily: Vec<DailyUsage>,
    pub by_activity: Vec<BreakdownRow>,
    pub by_model: Vec<ModelRow>,
    pub by_place: Vec<BreakdownRow>,
    pub costliest: Vec<ConversationRow>,
    pub unpriced: UnpricedNote,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct CallRow {
    pub at: DateTime<Utc>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UsageLinkKind {
    Chat,
    TaskRun,
    Dream,
    Topic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct UsageLink {
    pub kind: UsageLinkKind,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NoLinkReason {
    NoWebPage,
    Deleted,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
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
    fn activity_round_trips_through_its_name() {
        for a in UsageActivity::ALL {
            assert_eq!(a.as_str().parse::<UsageActivity>(), Ok(a));
            assert_eq!(serde_json::to_value(a).unwrap(), a.as_str());
        }
        assert!("nope".parse::<UsageActivity>().is_err());
    }
}
