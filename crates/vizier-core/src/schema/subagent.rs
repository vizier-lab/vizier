//! Subagents as temporary workers (spec 059): one task done for the main agent, on its own
//! `Subagent` conversation, recorded as one `subagent` row.

use chrono::{DateTime, Local};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::{AgentId, VizierChannelId, VizierSession};

/// The handle the main agent receives, e.g. `s_k3x9a1b2`.
pub type SubagentId = String;

/// A worker's time limit when the spawn names none.
pub const DEFAULT_TIMEOUT_SECS: u64 = 600;
/// The longest a worker may run (its time spent asking does not count).
pub const MAX_TIMEOUT_SECS: u64 = 3600;
/// A task, a reply or a question is refused above this many bytes.
pub const MAX_TASK_BYTES: usize = 16 * 1024;
/// `ask_main_agent`'s question is refused above this many bytes.
pub const MAX_QUESTION_BYTES: usize = 4 * 1024;
/// A worker's stored answer is cut at this many characters.
pub const MAX_RESULT_CHARS: usize = 20_000;
/// The task as a summary shows it.
pub const SUMMARY_TASK_CHARS: usize = 200;
/// The result as the session frame carries it.
pub const SNAPSHOT_RESULT_CHARS: usize = 500;

fn base36_id(prefix: &str) -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut n = uuid::Uuid::new_v4().as_u128();
    let mut out = String::with_capacity(prefix.len() + 8);
    out.push_str(prefix);
    for _ in 0..8 {
        out.push(ALPHABET[(n % 36) as usize] as char);
        n /= 36;
    }
    out
}

/// `s_` plus 8 lowercase base36 characters, like a detached script's `d_` id.
pub fn new_subagent_id() -> SubagentId {
    base36_id("s_")
}

/// One `spawn_subagents` call's group, `g_` plus 8 base36 characters.
pub fn new_group_id() -> String {
    base36_id("g_")
}

/// `answered`, `failed`, `timed_out` and `stopped` are final. `asking` returns to `running` only
/// through a reply. Every transition is one guarded update, so of two racing causes exactly one
/// wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum SubagentState {
    Running,
    Asking,
    Answered,
    Failed,
    TimedOut,
    Stopped,
}

impl SubagentState {
    pub const ALL: [SubagentState; 6] = [
        Self::Running,
        Self::Asking,
        Self::Answered,
        Self::Failed,
        Self::TimedOut,
        Self::Stopped,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Asking => "asking",
            Self::Answered => "answered",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
            Self::Stopped => "stopped",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.as_str() == value)
    }

    pub fn is_final(&self) -> bool {
        !matches!(self, Self::Running | Self::Asking)
    }

    /// What `wait_subagents` counts as settled: anything but `running`, so a main agent waiting
    /// for all its workers never waits on one that is waiting for it.
    pub fn is_settled(&self) -> bool {
        !matches!(self, Self::Running)
    }

    /// The states whose newest outcome is handed to the main agent (FR-024). `stopped` never
    /// is: the main agent stopped it, or the conversation is gone.
    pub fn carries_outcome(&self) -> bool {
        matches!(self, Self::Asking | Self::Answered | Self::Failed | Self::TimedOut)
    }
}

/// Why a worker is `stopped`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum SubagentStopReason {
    ByAgent,
    ConversationDeleted,
    AgentDeleted,
}

impl SubagentStopReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ByAgent => "by_agent",
            Self::ConversationDeleted => "conversation_deleted",
            Self::AgentDeleted => "agent_deleted",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::ByAgent, Self::ConversationDeleted, Self::AgentDeleted]
            .into_iter()
            .find(|reason| reason.as_str() == value)
    }
}

/// One row of `subagent`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subagent {
    pub id: SubagentId,
    /// The main agent, which is also the agent the worker's turns run on.
    pub agent_id: AgentId,
    /// The conversation it was spawned in: it bounds the worker's life and receives deliveries.
    pub hiring_session: VizierSession,
    /// Set when the spawn ran inside a detached script.
    pub parent_detached_id: Option<String>,
    pub group_id: String,
    /// 1-based position within its group.
    pub position: u32,
    pub group_size: u32,
    /// The worker's own `Subagent` topic.
    pub worker_topic: String,
    pub task: String,
    pub timeout_secs: u64,
    pub state: SubagentState,
    pub stop_reason: Option<SubagentStopReason>,
    /// The open question while `asking`; the last one asked afterwards.
    pub question: Option<String>,
    pub result: Option<String>,
    pub result_truncated: bool,
    pub error: Option<String>,
    /// +1 on every settle and every ask.
    pub outcome_seq: i64,
    /// The newest outcome the main agent read through a wait or a read.
    pub seen_seq: i64,
    /// The newest outcome delivered into the hiring conversation.
    pub delivered_seq: i64,
    /// While `running`: when its time limit runs out.
    pub deadline_at: Option<DateTime<Local>>,
    /// Set on entering `asking`: how much of its time limit was left.
    pub remaining_ms: Option<i64>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub ended_at: Option<DateTime<Local>>,
}

impl Subagent {
    /// The worker's own conversation: a turn of the main agent on a `Subagent` channel.
    pub fn worker_session(&self) -> VizierSession {
        worker_session(&self.agent_id, &self.worker_topic)
    }

    /// Whether the newest outcome still has to reach the main agent (data-model.md).
    pub fn is_deliverable(&self) -> bool {
        self.state.carries_outcome() && self.seen_seq.max(self.delivered_seq) < self.outcome_seq
    }

    /// Seconds from spawn to the end, or to `now` while unfinished.
    pub fn elapsed_secs(&self, now: DateTime<Local>) -> u64 {
        let end = self.ended_at.unwrap_or(now);
        (end - self.created_at).num_seconds().max(0) as u64
    }

    /// What the tools return for one worker. `full` keeps the whole task and result.
    pub fn summary(&self, full: bool) -> SubagentSummary {
        let (task, _) = if full {
            (self.task.clone(), false)
        } else {
            cut(&self.task, SUMMARY_TASK_CHARS)
        };
        SubagentSummary {
            id: self.id.clone(),
            position: self.position,
            group_size: self.group_size,
            task,
            state: self.state,
            stop_reason: self.stop_reason,
            question: matches!(self.state, SubagentState::Asking)
                .then(|| self.question.clone())
                .flatten(),
            result: self.result.clone(),
            truncated: self.result_truncated,
            error: self.error.clone(),
            started_at: self.created_at,
            ended_at: self.ended_at,
            topic: self.worker_topic.clone(),
        }
    }

    /// The `subagent` session frame: small enough to push on every change.
    pub fn snapshot(&self) -> SubagentSnapshot {
        let mut summary = self.summary(false);
        if let Some(result) = &summary.result {
            let (cut_result, was_cut) = cut(result, SNAPSHOT_RESULT_CHARS);
            summary.result = Some(cut_result);
            summary.truncated |= was_cut;
        }
        SubagentSnapshot {
            summary,
            group: self.group_id.clone(),
            parent_detached_id: self.parent_detached_id.clone(),
            handed_over: self.handed_over(),
            updated_at: self.updated_at,
        }
    }

    /// One worker for a person's detail read: the whole task and result, nothing cut.
    pub fn detail(&self) -> SubagentSnapshot {
        SubagentSnapshot {
            summary: self.summary(true),
            group: self.group_id.clone(),
            parent_detached_id: self.parent_detached_id.clone(),
            handed_over: self.handed_over(),
            updated_at: self.updated_at,
        }
    }

    /// Whether the main agent has its newest outcome, by a wait, a read or a delivery.
    pub fn handed_over(&self) -> bool {
        self.outcome_seq > 0 && self.seen_seq.max(self.delivered_seq) >= self.outcome_seq
    }
}

/// The worker conversation of `topic` for `agent`.
pub fn worker_session(agent: &AgentId, topic: &str) -> VizierSession {
    VizierSession(agent.clone(), VizierChannelId::Subagent, Some(topic.to_string()))
}

/// Cut `text` to `limit` characters with a marker. Returns whether it cut.
pub fn cut(text: &str, limit: usize) -> (String, bool) {
    match text.char_indices().nth(limit) {
        Some((at, _)) => (format!("{}… [cut]", &text[..at]), true),
        None => (text.to_string(), false),
    }
}

/// One worker as the subagent tools return it (contracts/subagent-tools.md).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SubagentSummary {
    pub id: SubagentId,
    pub position: u32,
    pub group_size: u32,
    pub task: String,
    pub state: SubagentState,
    pub stop_reason: Option<SubagentStopReason>,
    /// The open question, only while `asking`.
    pub question: Option<String>,
    pub result: Option<String>,
    pub truncated: bool,
    pub error: Option<String>,
    pub started_at: DateTime<Local>,
    pub ended_at: Option<DateTime<Local>>,
    /// The worker's conversation topic, where its full history stays.
    pub topic: String,
}

/// The `subagent` session frame, published on the hiring conversation after every change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SubagentSnapshot {
    #[serde(flatten)]
    pub summary: SubagentSummary,
    pub group: String,
    pub parent_detached_id: Option<String>,
    /// The main agent has the newest outcome (an answer, a failure or a question).
    #[serde(default)]
    pub handed_over: bool,
    pub updated_at: DateTime<Local>,
}

/// The first request a worker receives (`VizierRequestContent::SubagentTask`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SubagentBrief {
    pub subagent_id: SubagentId,
    pub position: u32,
    pub group_size: u32,
    pub text: String,
}

/// The main agent's answer to a worker's question (`VizierRequestContent::SubagentReply`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SubagentReplyText {
    pub subagent_id: SubagentId,
    pub text: String,
}

/// A worker row for tests elsewhere in the crate.
#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;

    pub(crate) fn row(state: SubagentState) -> Subagent {
        let now = crate::time::now();
        Subagent {
            id: "s_1".into(),
            agent_id: "a".into(),
            hiring_session: VizierSession(
                "a".into(),
                VizierChannelId::HTTP("dani".into(), "chat".into()),
                Some("t".into()),
            ),
            parent_detached_id: None,
            group_id: "g_1".into(),
            position: 1,
            group_size: 2,
            worker_topic: "w".into(),
            task: "do it".into(),
            timeout_secs: 600,
            state,
            stop_reason: None,
            question: None,
            result: None,
            result_truncated: false,
            error: None,
            outcome_seq: 0,
            seen_seq: 0,
            delivered_seq: 0,
            deadline_at: None,
            remaining_ms: None,
            created_at: now,
            updated_at: now,
            ended_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::row;
    use super::*;

    #[test]
    fn ids_have_their_prefix_and_eight_base36() {
        for id in [new_subagent_id(), new_group_id()] {
            assert_eq!(id.len(), 10, "{id}");
            assert!(id[2..].bytes().all(|b| b.is_ascii_digit() || b.is_ascii_lowercase()), "{id}");
        }
        assert!(new_subagent_id().starts_with("s_"));
        assert!(new_group_id().starts_with("g_"));
        assert_ne!(new_subagent_id(), new_subagent_id());
    }

    #[test]
    fn states_and_reasons_round_trip() {
        for state in SubagentState::ALL {
            assert_eq!(SubagentState::parse(state.as_str()), Some(state));
            assert_eq!(serde_json::to_value(state).unwrap(), state.as_str());
        }
        for reason in [
            SubagentStopReason::ByAgent,
            SubagentStopReason::ConversationDeleted,
            SubagentStopReason::AgentDeleted,
        ] {
            assert_eq!(SubagentStopReason::parse(reason.as_str()), Some(reason));
        }
        assert!(SubagentState::Asking.is_settled() && !SubagentState::Asking.is_final());
        assert!(!SubagentState::Running.is_settled());
        assert!(!SubagentState::Stopped.carries_outcome());
    }

    #[test]
    fn deliverable_means_an_unseen_outcome_in_a_state_that_carries_one() {
        let mut worker = row(SubagentState::Answered);
        worker.outcome_seq = 1;
        assert!(worker.is_deliverable());
        worker.seen_seq = 1;
        assert!(!worker.is_deliverable(), "seen through a wait");
        worker.seen_seq = 0;
        worker.delivered_seq = 1;
        assert!(!worker.is_deliverable(), "already delivered");
        worker.state = SubagentState::Asking;
        worker.outcome_seq = 2;
        assert!(worker.is_deliverable(), "a second question is a new outcome");
        worker.state = SubagentState::Stopped;
        assert!(!worker.is_deliverable());
    }

    #[test]
    fn the_summary_shows_a_question_only_while_asking_and_cuts_the_task() {
        let mut worker = row(SubagentState::Asking);
        worker.question = Some("which?".into());
        worker.task = "x".repeat(SUMMARY_TASK_CHARS + 10);
        assert_eq!(worker.summary(false).question.as_deref(), Some("which?"));
        assert!(worker.summary(false).task.ends_with("[cut]"));
        assert_eq!(worker.summary(true).task.len(), SUMMARY_TASK_CHARS + 10);
        worker.state = SubagentState::Running;
        assert_eq!(worker.summary(false).question, None);
    }

    #[test]
    fn the_worker_session_is_a_subagent_channel_of_the_same_agent() {
        let worker = row(SubagentState::Running);
        assert_eq!(
            worker.worker_session(),
            VizierSession("a".into(), VizierChannelId::Subagent, Some("w".into()))
        );
    }
}
