use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::{AgentId, TopicId, VizierSession};

/// A short random id, e.g. `b-7f3a9c`, quoted in the tool's acknowledgement and in the report.
pub type BackgroundJobId = String;

/// `b-` plus 6 lowercase hex characters taken from a v4 uuid.
pub fn new_job_id() -> BackgroundJobId {
    let hex = uuid::Uuid::new_v4().simple().to_string();
    format!("b-{}", &hex[..6])
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Batch,
    Delegation,
}

impl JobKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Batch => "batch",
            Self::Delegation => "delegation",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "batch" => Some(Self::Batch),
            "delegation" => Some(Self::Delegation),
            _ => None,
        }
    }
}

/// `running` and `reporting` are the in-flight states. Cancel and the runner race on one
/// guarded transition out of `running`, which is what makes a job end exactly once: either
/// reported or cancelled, never both.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Running,
    Reporting,
    Reported,
    Undelivered,
    Cancelled,
    Interrupted,
}

impl JobState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Reporting => "reporting",
            Self::Reported => "reported",
            Self::Undelivered => "undelivered",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "running" => Some(Self::Running),
            "reporting" => Some(Self::Reporting),
            "reported" => Some(Self::Reported),
            "undelivered" => Some(Self::Undelivered),
            "cancelled" => Some(Self::Cancelled),
            "interrupted" => Some(Self::Interrupted),
            _ => None,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PieceState {
    Running,
    Answered,
    Failed,
    TimedOut,
    Cancelled,
    Interrupted,
}

impl PieceState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Answered => "answered",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "running" => Some(Self::Running),
            "answered" => Some(Self::Answered),
            "failed" => Some(Self::Failed),
            "timed_out" => Some(Self::TimedOut),
            "cancelled" => Some(Self::Cancelled),
            "interrupted" => Some(Self::Interrupted),
            _ => None,
        }
    }

    /// How the state reads in a report heading and in `list_background_jobs`.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Answered => "answered",
            Self::Failed => "failed",
            Self::TimedOut => "timed out",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct BackgroundJob {
    pub id: BackgroundJobId,
    pub kind: JobKind,
    /// The conversation that launched the job and receives its report.
    pub origin: VizierSession,
    /// The `background_depth` of the turn that launched the job.
    pub depth: u8,
    pub timeout_secs: u64,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub state: JobState,
    pub cancelled_by: Option<Canceller>,
    pub reason: Option<String>,
    pub pieces: Vec<BackgroundPiece>,
}

impl BackgroundJob {
    /// The target agent of a delegation; `None` for a batch.
    pub fn delegated_to(&self) -> Option<AgentId> {
        match self.kind {
            JobKind::Delegation => self.pieces.first().map(|piece| piece.session.0.clone()),
            JobKind::Batch => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct BackgroundPiece {
    /// 0-based position in the original task order.
    pub ordinal: u32,
    pub prompt: String,
    /// `(executor_agent, Subagent | InterAgent([origin, target]), Some(fresh uuid))`.
    pub session: VizierSession,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub state: PieceState,
    pub reason: Option<String>,
}

/// What `VizierRequestContent::BackgroundReport` carries; rendered for the model by `Display`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct BackgroundReport {
    pub job_id: BackgroundJobId,
    pub kind: JobKind,
    pub delegated_to: Option<AgentId>,
    /// One per piece, in ordinal order.
    pub entries: Vec<ReportEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct ReportEntry {
    pub ordinal: u32,
    pub prompt: String,
    /// Never `Running`/`Interrupted`; `Cancelled` only in a cancel's tool result.
    pub state: PieceState,
    /// The answer (truncated) or the reason it did not answer.
    pub text: String,
    pub truncated: bool,
}

/// The WebUI wire shape (`contracts/http-api.md`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct BackgroundJobSnapshot {
    pub id: BackgroundJobId,
    pub kind: JobKind,
    pub delegated_to: Option<AgentId>,
    pub state: JobState,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub timeout_secs: u64,
    pub cancelled_by: Option<Canceller>,
    pub reason: Option<String>,
    pub pieces: Vec<BackgroundPieceSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct BackgroundPieceSnapshot {
    pub ordinal: u32,
    pub prompt: String,
    pub state: PieceState,
    pub reason: Option<String>,
    pub agent_id: AgentId,
    pub topic: Option<TopicId>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl From<&BackgroundJob> for BackgroundJobSnapshot {
    fn from(job: &BackgroundJob) -> Self {
        Self {
            id: job.id.clone(),
            kind: job.kind,
            delegated_to: job.delegated_to(),
            state: job.state,
            created_at: job.created_at,
            finished_at: job.finished_at,
            timeout_secs: job.timeout_secs,
            cancelled_by: job.cancelled_by.clone(),
            reason: job.reason.clone(),
            pieces: job
                .pieces
                .iter()
                .map(|piece| BackgroundPieceSnapshot {
                    ordinal: piece.ordinal,
                    prompt: piece.prompt.clone(),
                    state: piece.state,
                    reason: piece.reason.clone(),
                    agent_id: piece.session.0.clone(),
                    topic: piece.session.2.clone(),
                    started_at: piece.started_at,
                    finished_at: piece.finished_at,
                })
                .collect(),
        }
    }
}
