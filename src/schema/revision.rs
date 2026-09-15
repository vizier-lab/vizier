//! Version-history value types shared by CORE and memory documents.
//!
//! These describe *a save* (`RevisionOrigin`) and *a diff* (`RevisionDiff`) independent of
//! what was saved. CORE and memory history are two separate implementations
//! (`storage::sqlite::core_revision` / `storage::sqlite::memory_revision`); only these
//! kind-agnostic types and the `storage::diff` engine are shared between them.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    channels::http::auth::{AuthMethod, AuthenticatedUser},
    schema::{VizierChannelId, VizierSession},
};

/// Who performed a save.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RevisionActor {
    Agent,
    User { user_id: String, username: String },
    System,
}

impl RevisionActor {
    /// Persisted form: `(actor_kind, actor_id, actor_name)` columns.
    pub fn to_columns(&self) -> (&'static str, Option<&str>, Option<&str>) {
        match self {
            RevisionActor::Agent => ("agent", None, None),
            RevisionActor::User { user_id, username } => {
                ("user", Some(user_id.as_str()), Some(username.as_str()))
            }
            RevisionActor::System => ("system", None, None),
        }
    }

    pub fn from_columns(kind: &str, id: Option<String>, name: Option<String>) -> Self {
        match kind {
            "user" => RevisionActor::User {
                user_id: id.unwrap_or_default(),
                username: name.unwrap_or_default(),
            },
            "system" => RevisionActor::System,
            _ => RevisionActor::Agent,
        }
    }
}

/// What caused a save.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RevisionTrigger {
    Conversation,
    Dream,
    #[serde(rename = "webui")]
    WebUi,
    Api,
    Import,
    Rollback { restored_from: i64 },
    Baseline,
}

impl RevisionTrigger {
    /// Persisted form: `(trigger, restored_from)` columns.
    pub fn to_columns(&self) -> (&'static str, Option<i64>) {
        match self {
            RevisionTrigger::Conversation => ("conversation", None),
            RevisionTrigger::Dream => ("dream", None),
            RevisionTrigger::WebUi => ("webui", None),
            RevisionTrigger::Api => ("api", None),
            RevisionTrigger::Import => ("import", None),
            RevisionTrigger::Rollback { restored_from } => ("rollback", Some(*restored_from)),
            RevisionTrigger::Baseline => ("baseline", None),
        }
    }

    pub fn from_columns(trigger: &str, restored_from: Option<i64>) -> Self {
        match trigger {
            "dream" => RevisionTrigger::Dream,
            "webui" => RevisionTrigger::WebUi,
            "api" => RevisionTrigger::Api,
            "import" => RevisionTrigger::Import,
            "rollback" => RevisionTrigger::Rollback {
                restored_from: restored_from.unwrap_or(0),
            },
            "baseline" => RevisionTrigger::Baseline,
            _ => RevisionTrigger::Conversation,
        }
    }
}

/// Provenance attached to every recorded revision. Built at the boundary that knows who is
/// saving (agent tools via `from_session`, HTTP handlers via `from_user`) and threaded through
/// the storage write signatures so the single write path per document kind can record it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RevisionOrigin {
    pub actor: RevisionActor,
    pub trigger: RevisionTrigger,
}

impl RevisionOrigin {
    pub fn system(trigger: RevisionTrigger) -> Self {
        Self {
            actor: RevisionActor::System,
            trigger,
        }
    }

    pub fn with_trigger(mut self, trigger: RevisionTrigger) -> Self {
        self.trigger = trigger;
        self
    }

    /// Agent tools: the agent is the actor; a dream-cycle session maps to `Dream`, every other
    /// session kind is a `Conversation`.
    pub fn from_session(session: &VizierSession) -> Self {
        let trigger = match &session.1 {
            VizierChannelId::Dream(..) => RevisionTrigger::Dream,
            _ => RevisionTrigger::Conversation,
        };
        Self {
            actor: RevisionActor::Agent,
            trigger,
        }
    }

    /// HTTP handlers: the authenticated user is the actor; a JWT bearer token means the WebUI,
    /// an API key means a programmatic caller.
    pub fn from_user(user: &AuthenticatedUser) -> Self {
        let trigger = match user.auth_method {
            AuthMethod::Jwt => RevisionTrigger::WebUi,
            AuthMethod::ApiKey => RevisionTrigger::Api,
        };
        Self {
            actor: RevisionActor::User {
                user_id: user.user_id.clone(),
                username: user.username.clone(),
            },
            trigger,
        }
    }
}

/// One line of a diff hunk. `op` is `"equal"`, `"insert"` or `"delete"`; line numbers are
/// 1-based and `None` on the side the line does not exist on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DiffLine {
    pub op: String,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<DiffLine>,
}

/// Changes introduced by `to_seq` relative to `from_seq`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RevisionDiff {
    pub from_seq: i64,
    pub to_seq: i64,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RollbackResponse {
    /// `true` when the restored content was already identical to the current one, so no new
    /// revision was appended.
    pub no_change: bool,
    pub new_seq: Option<i64>,
    pub restored_from: i64,
}

// ---- CORE API types ----

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CoreRevisionSummary {
    pub seq: i64,
    pub actor: RevisionActor,
    pub trigger: RevisionTrigger,
    pub created_at: DateTime<Utc>,
    pub is_current: bool,
    pub size_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CoreRevision {
    pub seq: i64,
    pub actor: RevisionActor,
    pub trigger: RevisionTrigger,
    pub created_at: DateTime<Utc>,
    pub is_current: bool,
    pub size_bytes: usize,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PaginatedCoreRevisions {
    pub revisions: Vec<CoreRevisionSummary>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

// ---- Memory API types ----

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MemoryRevisionSummary {
    pub seq: i64,
    pub deleted: bool,
    pub actor: RevisionActor,
    pub trigger: RevisionTrigger,
    pub created_at: DateTime<Utc>,
    pub is_current: bool,
    pub size_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MemoryRevision {
    pub seq: i64,
    pub deleted: bool,
    pub actor: RevisionActor,
    pub trigger: RevisionTrigger,
    pub created_at: DateTime<Utc>,
    pub is_current: bool,
    pub size_bytes: usize,
    /// Canonical snapshot text; `None` on a deletion entry.
    pub content: Option<String>,
    pub title: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PaginatedMemoryRevisions {
    pub revisions: Vec<MemoryRevisionSummary>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::DreamStage;

    fn base_session() -> VizierSession {
        VizierSession("agent-1".into(), VizierChannelId::System, None)
    }

    #[test]
    fn dream_session_maps_to_dream_trigger() {
        let session = VizierSession(
            "agent-1".into(),
            VizierChannelId::Dream(Box::new(base_session()), DreamStage::Extraction),
            None,
        );
        let origin = RevisionOrigin::from_session(&session);
        assert_eq!(origin.actor, RevisionActor::Agent);
        assert_eq!(origin.trigger, RevisionTrigger::Dream);
    }

    #[test]
    fn non_dream_sessions_map_to_conversation() {
        for channel in [
            VizierChannelId::System,
            VizierChannelId::DiscordChanel(1),
            VizierChannelId::TelegramChannel(2),
            VizierChannelId::HTTP("u".into(), "s".into()),
            VizierChannelId::Subagent,
        ] {
            let origin = RevisionOrigin::from_session(&VizierSession("a".into(), channel, None));
            assert_eq!(origin.trigger, RevisionTrigger::Conversation);
            assert_eq!(origin.actor, RevisionActor::Agent);
        }
    }

    #[test]
    fn with_trigger_replaces_only_the_trigger() {
        let origin = RevisionOrigin::from_session(&base_session())
            .with_trigger(RevisionTrigger::Rollback { restored_from: 3 });
        assert_eq!(origin.actor, RevisionActor::Agent);
        assert_eq!(origin.trigger, RevisionTrigger::Rollback { restored_from: 3 });
    }

    #[test]
    fn serializes_snake_case_tagged() {
        let json = serde_json::to_string(&RevisionTrigger::Rollback { restored_from: 2 }).unwrap();
        assert_eq!(json, r#"{"type":"rollback","restored_from":2}"#);
        let json = serde_json::to_string(&RevisionActor::Agent).unwrap();
        assert_eq!(json, r#"{"type":"agent"}"#);
        let json = serde_json::to_string(&RevisionTrigger::WebUi).unwrap();
        assert_eq!(json, r#"{"type":"webui"}"#);
    }
}
