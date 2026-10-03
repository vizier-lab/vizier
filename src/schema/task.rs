use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


use crate::schema::AgentId;
use crate::utils::markdown::MarkdownDoc;

/// Who wanted a task to exist.
///
/// The one distinction that matters — somebody asked, versus the agent set this
/// up on its own initiative — made unforgeable in the type. `User`'s payload is
/// deliberately not an account: most people reaching an agent have none, and the
/// chat channels already synthesise an identity string for them.
#[derive(Debug, Serialize, Clone, PartialEq, Eq, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Requester {
    /// Someone asked for this task, identified as the channel they reached the
    /// agent on knows them — `"@dani (DiscordId: 182…)"`, a web username.
    User(String),
    /// The agent set this task up itself, with no one asking.
    Agent(AgentId),
}

/// Deserializes from the tagged form it serializes to — `{"user": "..."}` /
/// `{"agent": "..."}` — and additionally from a bare string, which is read as `User`.
///
/// The bare-string arm is the legacy `user: alice` that a pre-`Requester` task carries,
/// whether in a sqlite `task.data` blob or in a filesystem deployment's task frontmatter.
/// The startup migration rewrites sqlite rows into the tagged form, but the filesystem
/// backend is read *before* that migration runs and one unparseable file would otherwise
/// abort the whole fs-to-sqlite task migration. A bare string cannot be an agent's own
/// initiative, since nothing recorded before this change distinguished one.
impl<'de> Deserialize<'de> for Requester {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Tagged(Tagged),
            Legacy(String),
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum Tagged {
            User(String),
            Agent(String),
        }

        Ok(match Wire::deserialize(deserializer)? {
            Wire::Tagged(Tagged::User(identity)) => Self::User(identity),
            Wire::Tagged(Tagged::Agent(agent_id)) => Self::Agent(agent_id),
            Wire::Legacy(identity) => Self::User(identity),
        })
    }
}

impl Requester {
    /// How the requester reads in a request's frontmatter: the person's identity,
    /// or `self` where the task was the agent's own initiative.
    pub fn to_frontmatter(&self) -> String {
        match self {
            Self::User(identity) => identity.clone(),
            Self::Agent(_) => "self".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema, utoipa::ToSchema, MarkdownDoc)]
pub struct Task {
    pub slug: String,
    pub requester: Requester,
    pub agent_id: String,
    pub title: String,
    #[markdown(content)]
    pub instruction: String,
    pub is_active: bool,
    pub schedule: TaskSchedule,
    pub last_executed_at: Option<DateTime<Utc>>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema, utoipa::ToSchema)]
pub enum TaskSchedule {
    CronTask(String),
    OneTimeTask(DateTime<Utc>),
}

/// One firing of a task.
///
/// The row points at the conversation the run wrote into and deliberately does
/// **not** hold the response text — a second copy would drift from the
/// conversation it came from. The response is sliced from `session_history` when
/// asked for.
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema, utoipa::ToSchema)]
pub struct TaskRun {
    /// Monotonic; the low half of the page cursor.
    pub id: i64,
    pub agent_id: AgentId,
    pub task_slug: String,
    /// The run's address, and the high half of the cursor.
    pub ran_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    /// The conversation this run wrote into.
    pub session_key: String,
    pub state: TaskRunState,
}

/// Terminal states are terminal — nothing re-opens a closed run. *Not yet run*
/// is the **absence** of rows for a task, not a state.
///
/// `Answered` means the agent produced a response, not that the news was good: a
/// run whose response reports a failure is still `Answered`.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskRunState {
    Running,
    Answered,
    NoResponse,
    Interrupted,
}

impl TaskRunState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Answered => "answered",
            Self::NoResponse => "no_response",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "running" => Some(Self::Running),
            "answered" => Some(Self::Answered),
            "no_response" => Some(Self::NoResponse),
            "interrupted" => Some(Self::Interrupted),
            _ => None,
        }
    }
}
