//! Triggers (`specs/071-triggers/`): an agent's standing rule "when this happens, do this".
//!
//! A trigger receives **events** from its kind (a webhook request, a monitor's finding). An event
//! that passes the filter, and triage where it is on, waits for a **run**: one unattended turn on
//! the trigger's own conversation over up to [`MAX_EVENTS_PER_RUN`] waiting events, whose reply
//! Vizier delivers to the person who set the trigger up.

use chrono::{DateTime, Local};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::{AgentId, Requester};

/// How many waiting events one run takes (FR-007), and how many one triage call judges (FR-044).
pub const MAX_EVENTS_PER_RUN: usize = 20;
/// Important events that may wait for a run (FR-015).
pub const MAX_WAITING_EVENTS: usize = 100;
/// Events that may wait for triage (FR-044).
pub const MAX_PENDING_TRIAGE: usize = 500;
/// Runs an event is given before it is marked failed (FR-049).
pub const MAX_EVENT_ATTEMPTS: u32 = 2;
/// The default run limit per hour (FR-016).
pub const DEFAULT_HOURLY_LIMIT: u32 = 20;
/// The default triage calls per hour (FR-044).
pub const DEFAULT_TRIAGE_CALLS_PER_HOUR: u32 = 200;
/// The default summary window (FR-050).
pub const DEFAULT_SUMMARY_HOURS: u32 = 5;
/// How long an event's payload is kept (FR-051).
pub const PAYLOAD_RETENTION_DAYS: i64 = 3;
/// The cap on one stored payload, after redaction (FR-009).
pub const MAX_STORED_PAYLOAD_BYTES: usize = 256 * 1024;

/// What happens to events that arrive while a run is in progress (FR-015).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum OverlapPolicy {
    /// They wait, and the next run takes them together.
    #[default]
    Wait,
    /// They are dropped, before triage (FR-044).
    Drop,
}

impl OverlapPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Wait => "wait",
            Self::Drop => "drop",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "drop" => Self::Drop,
            _ => Self::Wait,
        }
    }
}

/// What a full queue does with a new event; chosen by the kind (FR-015, research R14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhenFull {
    /// The oldest waiting event is dropped. A webhook cannot hold its sender, who was answered.
    DropOldest,
    /// The new event is refused and its source keeps its state, so it reports it again.
    RefuseNew,
}

/// Optional triage by a model the person chose (FR-042).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TriageConfig {
    /// What counts as important, in the person's words.
    pub criteria: String,
    /// The provider and model that judge. `None` until a person approves an agent-made trigger,
    /// which is when the model is chosen.
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default = "default_triage_calls")]
    pub calls_per_hour: u32,
}

fn default_triage_calls() -> u32 {
    DEFAULT_TRIAGE_CALLS_PER_HOUR
}

impl TriageConfig {
    /// Whether a model has been chosen.
    pub fn model(&self) -> Option<(&str, &str)> {
        Some((self.provider.as_deref()?, self.model.as_deref()?))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Trigger {
    /// Random and global: a webhook's address.
    pub id: String,
    pub agent_id: AgentId,
    /// Unique on the agent.
    pub slug: String,
    /// A trigger kind id: `webhook`, `monitor`, or an addon's.
    pub kind: String,
    pub title: String,
    pub instruction: String,
    /// The kind's settings. Secrets are encrypted at rest and masked in every response.
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub settings: serde_json::Value,
    pub requester: Requester,
    /// The person results and notices go to (FR-046): a user id, resolved when the trigger is
    /// created. `None` means the agent's owner.
    #[serde(default)]
    pub delivers_to: Option<String>,
    pub is_active: bool,
    /// Set for every trigger an agent creates, until a person approves it (FR-036).
    #[serde(default)]
    pub awaiting_approval: bool,
    /// Why the trigger needs a person's attention, such as a removed triage model.
    #[serde(default)]
    pub needs_attention: Option<String>,
    /// The tool categories its runs may use; `None` is every tool (FR-030).
    #[serde(default)]
    pub tool_scope: Option<Vec<String>>,
    #[serde(default)]
    pub overlap: OverlapPolicy,
    /// Runs per hour; `None` is no limit (FR-016).
    #[serde(default)]
    pub hourly_limit: Option<u32>,
    #[serde(default)]
    pub cooldown_secs: u32,
    #[serde(default)]
    pub triage: Option<TriageConfig>,
    /// A conversation reference the reply also goes to (FR-046).
    #[serde(default)]
    pub destination: Option<String>,
    /// Hours per summary; `None` is off (FR-050).
    #[serde(default)]
    pub summary_hours: Option<u32>,
    #[serde(default)]
    pub last_summary_at: Option<DateTime<Local>>,
    pub created_at: DateTime<Local>,
    #[serde(default)]
    pub last_fired_at: Option<DateTime<Local>>,
}

impl Trigger {
    /// Whether events are accepted: active and approved.
    pub fn accepts_events(&self) -> bool {
        self.is_active && !self.awaiting_approval
    }
}

/// What became of one event (FR-008).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    TriagePending,
    Unimportant,
    Waiting,
    Handled,
    Ignored,
    Dropped,
    Refused,
    Failed,
}

impl EventStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TriagePending => "triage_pending",
            Self::Unimportant => "unimportant",
            Self::Waiting => "waiting",
            Self::Handled => "handled",
            Self::Ignored => "ignored",
            Self::Dropped => "dropped",
            Self::Refused => "refused",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "triage_pending" => Self::TriagePending,
            "unimportant" => Self::Unimportant,
            "waiting" => Self::Waiting,
            "handled" => Self::Handled,
            "ignored" => Self::Ignored,
            "dropped" => Self::Dropped,
            "refused" => Self::Refused,
            "failed" => Self::Failed,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TriggerEvent {
    pub id: i64,
    pub trigger_id: String,
    pub agent_id: AgentId,
    pub received_at: DateTime<Local>,
    /// Redacted and capped; `None` on a collapsed drop record and after
    /// [`PAYLOAD_RETENTION_DAYS`].
    #[cfg_attr(feature = "utoipa", schema(value_type = Option<Object>))]
    pub payload: Option<serde_json::Value>,
    pub status: EventStatus,
    /// The filter, the triage verdict's reason, or why it was dropped.
    pub reason: Option<String>,
    /// Triage failed and the event was counted as important (FR-045).
    pub untriaged: bool,
    /// Events folded into this record (a collapsed drop record counts more than one).
    pub count: i64,
    pub attempts: u32,
    pub run_id: Option<i64>,
}

/// The state of one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum TriggerRunState {
    Running,
    Answered,
    NoResponse,
    Interrupted,
}

impl TriggerRunState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Answered => "answered",
            Self::NoResponse => "no_response",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "running" => Self::Running,
            "answered" => Self::Answered,
            "no_response" => Self::NoResponse,
            "interrupted" => Self::Interrupted,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TriggerRun {
    pub id: i64,
    pub trigger_id: String,
    pub agent_id: AgentId,
    /// The trigger's slug when the run started; the conversation is addressed by it.
    pub trigger_slug: String,
    pub started_at: DateTime<Local>,
    pub finished_at: Option<DateTime<Local>>,
    pub state: TriggerRunState,
    pub session_key: String,
    pub delivered_at: Option<DateTime<Local>>,
    /// Set when the run ended with "nothing to report" (FR-050).
    pub quiet_note: Option<String>,
}

/// How an event was received, as the webhook answers its sender (FR-023).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum AdmitOutcome {
    Accepted,
    Ignored,
    Dropped,
    Refused,
    Duplicate,
}

/// The result of recording one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Admission {
    pub event_id: i64,
    pub outcome: AdmitOutcome,
}

/// One monitor check, run inside the agent's instance (`specs/071-triggers` research R1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorCheck {
    pub trigger_slug: String,
    pub script: String,
    /// The state the script returned last time, bound as `input["state"]`.
    pub state: Option<serde_json::Value>,
    /// The scope the script's calls run under, already narrowed by the monitor (FR-028, FR-031).
    pub scope: Vec<String>,
    /// The wall-clock limit (FR-028).
    pub timeout_secs: u64,
}

/// What a check produced: the script's last expression, or why it failed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckOutcome {
    pub value: Option<serde_json::Value>,
    pub error: Option<String>,
    /// The tools the script called, by name, for the test result.
    #[serde(default)]
    pub calls: Vec<String>,
}

impl CheckOutcome {
    pub fn failed(error: impl Into<String>) -> Self {
        Self { value: None, error: Some(error.into()), calls: vec![] }
    }
}
