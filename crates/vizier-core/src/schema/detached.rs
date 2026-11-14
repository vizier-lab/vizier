//! Detached scripts (spec 058): an `execute_python` run with `detached: true`, which runs in
//! its own task while the agent continues and is delivered back to its conversation once.

use chrono::{DateTime, Local};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::{AgentId, VizierSession};

/// The handle the agent receives, e.g. `d_k3x9a1b2`.
pub type DetachedScriptId = String;

/// The whole script's time limit when the call names none.
pub const DEFAULT_TIMEOUT_SECS: u64 = 600;
/// The longest a detached script may run.
pub const MAX_TIMEOUT_SECS: u64 = 3600;
/// A turn at this `background_depth` may not detach (a script inside a sub-agent inside a
/// script, three times over).
pub const MAX_BACKGROUND_DEPTH: u8 = 3;
/// The stored outcome and the tool-call list are cut at this many bytes.
pub const MAX_OUTCOME_BYTES: usize = 32 * 1024;
/// Each stored tool-call argument preview is cut at this many characters.
pub const MAX_ARGUMENT_PREVIEW_CHARS: usize = 500;
/// The stored intent is cut at this many characters.
pub const MAX_INTENT_CHARS: usize = 500;

/// `d_` plus 8 lowercase base36 characters from a v4 uuid's random bits.
pub fn new_detached_id() -> DetachedScriptId {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut n = uuid::Uuid::new_v4().as_u128();
    let mut out = String::with_capacity(10);
    out.push_str("d_");
    for _ in 0..8 {
        out.push(ALPHABET[(n % 36) as usize] as char);
        n /= 36;
    }
    out
}

/// Every transition out of `running` is one guarded update, so of two racing causes
/// (a cancel and the script finishing, say) exactly one wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum DetachedScriptState {
    Running,
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
    Interrupted,
}

impl DetachedScriptState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "running" => Some(Self::Running),
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            "timed_out" => Some(Self::TimedOut),
            "cancelled" => Some(Self::Cancelled),
            "interrupted" => Some(Self::Interrupted),
            _ => None,
        }
    }

    /// How the state reads in a delivery and a notification.
    pub fn label(&self) -> &'static str {
        match self {
            Self::TimedOut => "timed out",
            other => other.as_str(),
        }
    }

    pub fn is_final(&self) -> bool {
        !matches!(self, Self::Running)
    }

    /// A final script that is not cancelled is delivered.
    pub fn is_deliverable(&self) -> bool {
        self.is_final() && !matches!(self, Self::Cancelled)
    }

    /// The `stopped` filter of `list_detached_scripts` and the panel's Stopped tab.
    pub fn is_stopped(&self) -> bool {
        matches!(self, Self::Cancelled | Self::Interrupted | Self::TimedOut)
    }
}

/// Who cancelled a script. Only the agent does in spec 058; a person's cancel is deferred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Canceller {
    Agent(AgentId),
    Person(String),
}

impl Canceller {
    /// The `cancelled_by` column: `agent:<id>` or `person:<username>`.
    pub fn to_column(&self) -> String {
        match self {
            Self::Agent(id) => format!("agent:{id}"),
            Self::Person(name) => format!("person:{name}"),
        }
    }

    pub fn from_column(value: &str) -> Option<Self> {
        if let Some(id) = value.strip_prefix("agent:") {
            return Some(Self::Agent(id.to_string()));
        }
        value
            .strip_prefix("person:")
            .map(|name| Self::Person(name.to_string()))
    }
}

/// One row of `detached_script`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetachedScript {
    pub id: DetachedScriptId,
    pub agent_id: AgentId,
    /// The conversation it was launched from, and where its delivery goes.
    pub session: VizierSession,
    /// The detached script whose sub-agent turn launched this one.
    pub parent_id: Option<DetachedScriptId>,
    /// `ToolContext.background_depth` of the launching turn.
    pub depth: u8,
    pub intent: String,
    pub state: DetachedScriptState,
    pub timeout_secs: u64,
    /// Redacted source.
    pub code: String,
    /// `ToolInvocationRecord`s so far, redacted, previews cut.
    pub tool_calls: serde_json::Value,
    pub created_at: DateTime<Local>,
    pub finished_at: Option<DateTime<Local>>,
    /// The redacted `ExecutionReport` as JSON, or a failure message as a JSON string.
    pub outcome: Option<serde_json::Value>,
    pub delivered_at: Option<DateTime<Local>>,
    pub cancelled_by: Option<Canceller>,
}

impl DetachedScript {
    pub fn tool_call_count(&self) -> usize {
        self.tool_calls.as_array().map_or(0, Vec::len)
    }

    /// Seconds from launch to the end, or to `now` while running.
    pub fn elapsed_secs(&self, now: DateTime<Local>) -> u64 {
        let end = self.finished_at.unwrap_or(now);
        (end - self.created_at).num_seconds().max(0) as u64
    }

    pub fn snapshot(&self) -> DetachedScriptSnapshot {
        DetachedScriptSnapshot {
            id: self.id.clone(),
            intent: self.intent.clone(),
            state: self.state,
            created_at: self.created_at,
            finished_at: self.finished_at,
            timeout_secs: self.timeout_secs,
            parent_id: self.parent_id.clone(),
            delivered: self.delivered_at.is_some(),
            tool_call_count: self.tool_call_count() as u32,
        }
    }

    pub fn detail(&self) -> DetachedScriptDetail {
        DetachedScriptDetail {
            snapshot: self.snapshot(),
            code: self.code.clone(),
            tool_calls: self.tool_calls.clone(),
            report: self.outcome.clone(),
        }
    }
}

/// The list route's rows and the `detached_script` session frame. Small enough to push on
/// every change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DetachedScriptSnapshot {
    pub id: DetachedScriptId,
    pub intent: String,
    pub state: DetachedScriptState,
    pub created_at: DateTime<Local>,
    pub finished_at: Option<DateTime<Local>>,
    pub timeout_secs: u64,
    pub parent_id: Option<DetachedScriptId>,
    pub delivered: bool,
    pub tool_call_count: u32,
}

/// One script as the panel draws it: the activity trail's run detail.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DetachedScriptDetail {
    #[serde(flatten)]
    pub snapshot: DetachedScriptSnapshot,
    pub code: String,
    /// `[{seq, name, arguments, ok, error?, duration_ms}]`.
    #[cfg_attr(feature = "utoipa", schema(value_type = Vec<Object>))]
    pub tool_calls: serde_json::Value,
    /// The `ExecutionReport` once final (or the failure message as a string).
    #[cfg_attr(feature = "utoipa", schema(value_type = Option<Object>))]
    pub report: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_d_and_eight_base36() {
        for _ in 0..100 {
            let id = new_detached_id();
            assert_eq!(id.len(), 10, "{id}");
            assert!(id.starts_with("d_"));
            assert!(id[2..].bytes().all(|b| b.is_ascii_digit() || b.is_ascii_lowercase()), "{id}");
        }
        assert_ne!(new_detached_id(), new_detached_id());
    }

    #[test]
    fn states_round_trip() {
        for state in [
            DetachedScriptState::Running,
            DetachedScriptState::Succeeded,
            DetachedScriptState::Failed,
            DetachedScriptState::TimedOut,
            DetachedScriptState::Cancelled,
            DetachedScriptState::Interrupted,
        ] {
            assert_eq!(DetachedScriptState::parse(state.as_str()), Some(state));
            assert_eq!(serde_json::to_value(state).unwrap(), state.as_str());
        }
        assert!(!DetachedScriptState::Cancelled.is_deliverable());
        assert!(DetachedScriptState::Interrupted.is_deliverable());
        assert!(!DetachedScriptState::Running.is_deliverable());
    }
}
