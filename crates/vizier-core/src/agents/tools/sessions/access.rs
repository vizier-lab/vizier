//! Who may be woken, and whose conversation it is (spec 067 research D9, D10).
//!
//! A wake starts a turn in a conversation of the **calling** agent: the agent that wakes is
//! the agent that is woken, so neither tool names an agent and the model never has to reason
//! about which agent owns a conversation. The one reachability question left is the person's:
//! one agent may serve several people, and a conversation belongs to the one it was had with.
//!
//! The tools are offered only where a person's access can be decided. A Discord or Telegram
//! conversation has no verified person on it — those platform identities are self-declared until
//! vizier-lab/vizier#116 — so nothing is offered there, and neither is anything started from
//! one (a detached script runs in its conversation's session, so the same test covers it).

use crate::agents::tools::ToolContext;
use crate::consultations::access::Consulter;
use crate::schema::VizierChannelId;
use crate::storage::VizierStorage;

/// Whether the session tools are offered in this turn (FR-018, research D10): not in a dream,
/// not inside a subagent worker, and not in a Discord or Telegram conversation or anything
/// started from one.
///
/// A scheduled run **is** offered them: a job's work belongs in the person's own conversation
/// (FR-026), and the agent's owner is the person whose access decides there.
pub fn may_wake(ctx: &ToolContext) -> bool {
    ctx.subagent_depth == 0
        && !matches!(
            ctx.session.1,
            VizierChannelId::Dream(..) | VizierChannelId::Subagent | VizierChannelId::Workspace(_)
        )
        && ctx.session.1.platform().is_none()
}

/// Why a wake was refused, as data the agent can act on (FR-024).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The address is not one the system knows.
    UnknownChannel,
    /// The conversation belongs to the system's own machinery, which owns it (FR-015).
    NotWakeable(String),
    /// No person whose access could decide: an agent without an owner, outside a person's chat
    /// (FR-018).
    NoOwner,
    /// The conversation belongs to somebody else (FR-017).
    OtherPerson(String),
    /// The hop limit is reached (FR-020).
    TooDeep,
    /// The call itself was malformed: an empty instruction, or a channel that has topics and
    /// was given none.
    BadRequest(String),
}

impl Refusal {
    pub fn to_json(&self) -> serde_json::Value {
        let (status, message) = match self {
            Self::UnknownChannel => (
                "unknown_channel",
                "that is not a conversation reference; list_sessions returns the ones you may use".into(),
            ),
            Self::NotWakeable(what) => (
                "not_wakeable",
                format!("that conversation is not yours to wake: {what}"),
            ),
            Self::NoOwner => (
                "no_owner",
                "this agent has no owner, and no person is in this conversation, so there is nobody whose conversation you may wake".into(),
            ),
            Self::OtherPerson(user) => (
                "other_person",
                format!("that conversation belongs to {user}, not to the person you are working for"),
            ),
            Self::TooDeep => (
                "too_deep",
                "nesting limit reached: this turn was itself started by a wake or by background work".into(),
            ),
            Self::BadRequest(why) => ("bad_request", why.clone()),
        };
        serde_json::json!({ "status": status, "message": message })
    }
}

/// The person whose access decides in this turn, resolved to their current role (so a revoked
/// permission is felt at once).
///
/// It is the consultation module's own resolution — the one existing helper that answers "whose
/// access decides in this turn" — so there is one rule for that, not two (research D9).
pub async fn person(
    storage: &VizierStorage,
    ctx: &ToolContext,
) -> crate::Result<Result<Consulter, Refusal>> {
    match crate::consultations::access::consulter(storage, ctx).await {
        Ok(Ok(who)) => Ok(Ok(who)),
        // The only refusal the consultation resolver produces is "no person"; the rest of its
        // reasons are about reaching another agent, which a wake never does.
        Ok(Err(_)) => Ok(Err(Refusal::NoOwner)),
        Err(e) => Err(e),
    }
}

/// Whether a target conversation belongs to the person this turn is working for (FR-017).
///
/// A Discord or Telegram conversation has no person of its own — the platform identity is
/// self-declared — so it belongs to nobody and is not refused here.
pub fn belongs_to_person(channel: &VizierChannelId, username: &str) -> bool {
    match channel.web_user() {
        Some(user) => user == username,
        None => true,
    }
}
