//! The session tools (spec 067): an agent lists its own conversations and wakes one.
//!
//! A wake starts a turn in a conversation of the **calling** agent — the agent that wakes is
//! the agent that is woken, so neither tool names an agent and the model never has to reason
//! about which agent owns a conversation (research D9). The two tools are a matched pair:
//! `list_sessions` answers "which of my conversations", `wake_session` answers "in that one".
//!
//! A wake is a call, not a record: there is no row, no handle and no state machine. What it does
//! is dispatch one request into a session — `transport.send_request` already takes any session —
//! and, when the caller asks to wait, read the reply frames the way the scheduler reads a task
//! run's (research D11, D12).
//!
//! The conversation reference is the channel's slug, the same string the conversation picker
//! lists and the WebUI's route carries, so a value taken from a listing is exactly what a wake
//! takes. It never names an agent: the agent is the one making the call.

pub mod access;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::agents::tools::{ToolCategory, ToolContext, VizierTool};
use crate::error::VizierError;
use crate::schema::{
    AgentId, VizierChannelId, VizierRequest, VizierRequestContent, VizierResponseContent,
    VizierSession,
};
use crate::storage::{session::SessionStorage, VizierStorage};
use crate::transport::VizierTransport;

/// The name the caller's own conversation goes by.
const CURRENT: &str = "current";

/// Whether a conversation on this channel may be woken (FR-014, FR-015): a WebUI conversation,
/// a project chat, a Discord channel or a Telegram chat.
///
/// Everything the system's own machinery owns — a scheduled run's, a subagent worker's, a
/// consultation's, a dream cycle's — is not. A foreign turn in one of those breaks the
/// machinery's own bookkeeping, so they are refused rather than shared.
pub fn is_wakeable(channel: &VizierChannelId) -> bool {
    matches!(
        channel,
        VizierChannelId::DiscordChanel(_)
            | VizierChannelId::TelegramChannel(_)
            | VizierChannelId::HTTP(..)
            | VizierChannelId::Project(..)
    )
}

/// Whether a channel has conversations of its own inside it, which a reference names by topic.
fn has_topics(channel: &VizierChannelId) -> bool {
    matches!(
        channel,
        VizierChannelId::HTTP(..) | VizierChannelId::Project(..)
    )
}

/// The channel a reference names, if it names one that may be woken.
///
/// This is the **only** place the reference vocabulary is read, so the set of wakeable
/// conversations is decided by what this parses and not by a second list that could drift
/// (research D2). A reference to a conversation the machinery owns is refused here too, with the
/// reason, rather than as an unknown address.
///
/// The username and the channel id are split at the **first** `__` after the prefix, so a
/// channel id containing `__` survives; a username containing one would not, and usernames do
/// not contain one.
fn wakeable_channel(slug: &str) -> Result<VizierChannelId, access::Refusal> {
    let channel = if let Some(id) = slug
        .strip_prefix("discord__")
        .and_then(|id| id.parse::<u64>().ok())
    {
        VizierChannelId::DiscordChanel(id)
    } else if let Some(id) = slug
        .strip_prefix("telegram__")
        .and_then(|id| id.parse::<i64>().ok())
    {
        VizierChannelId::TelegramChannel(id)
    } else if let Some((user, channel)) = slug
        .strip_prefix("http__")
        .and_then(|rest| rest.split_once("__"))
        .filter(|(user, channel)| !user.is_empty() && !channel.is_empty())
    {
        VizierChannelId::HTTP(user.to_string(), channel.to_string())
    } else if let Some((user, id)) = slug
        .strip_prefix("project__")
        .and_then(|rest| rest.split_once("__"))
        .filter(|(user, id)| !user.is_empty() && !id.is_empty())
    {
        VizierChannelId::Project(user.to_string(), id.to_string())
    } else {
        return Err(match machinery_kind(slug) {
            Some(what) => access::Refusal::NotWakeable(what.to_string()),
            None => access::Refusal::UnknownChannel,
        });
    };
    Ok(channel)
}

/// What kind of conversation the system's own machinery owns, when a reference names one.
///
/// The counterpart of [`is_wakeable`]: the conversations a foreign turn must never enter,
/// named so the agent is told which one owns it.
fn machinery_kind(slug: &str) -> Option<&'static str> {
    if slug.starts_with("task__") {
        Some("a scheduled run's own conversation, which the scheduler owns")
    } else if slug.starts_with("trigger__") {
        Some("a trigger run's own conversation, which the trigger owns")
    } else if slug.eq_ignore_ascii_case("SUBAGENT") {
        Some("a subagent worker's conversation, which the hiring turn owns")
    } else if slug.starts_with("consultation__") {
        Some("a consultation's conversation, which the consultation owns")
    } else if slug.starts_with("DREAM__") {
        Some("a dream cycle's conversation")
    } else if slug == "SYSTEM" {
        Some("the system's own channel")
    } else if slug.starts_with("inter_agent__") {
        Some("a delegation's conversation")
    } else if slug.starts_with("workspace__") {
        Some("a Workspace's conversation, which no longer exists")
    } else {
        None
    }
}

/// The topic a session on this channel has: the channels that have conversations of their own
/// need one, and the ones that do not never have one.
fn topic_for(channel: &VizierChannelId, topic_id: Option<String>) -> Option<String> {
    match channel {
        VizierChannelId::HTTP(..) | VizierChannelId::Project(..) => topic_id,
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// list_sessions
// ---------------------------------------------------------------------------------------------

pub struct ListSessions {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListSessionsArgs {
    /// Only conversations on this channel slug, e.g. "http__ana__vizier-webui".
    #[serde(default)]
    pub channel: Option<String>,

    /// Only conversations whose title contains this.
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionEntry {
    /// The channel slug, as `VizierChannelId::to_slug()` renders it. This is what a wake takes.
    pub channel: String,
    /// The topic, for the channels that have conversations of their own.
    pub topic_id: Option<String>,
    /// The conversation's title.
    pub title: String,
    /// Whether a turn is running in it.
    pub is_thinking: bool,
}

const LIST_DESCRIPTION: &str = "\
list the conversations you may work in. Each row carries the `channel` to hand to \
`wake_session` and the `topic_id` it needs; your own conversation is the one this turn is \
in. A conversation of somebody else is never listed.";

#[async_trait::async_trait]
impl VizierTool for ListSessions {
    type Input = ListSessionsArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Sessions)
    }

    fn name() -> String {
        "list_sessions".to_string()
    }

    fn description(&self) -> String {
        LIST_DESCRIPTION.into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        access::may_wake(ctx)
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let who = match access::person(&self.storage, _ctx).await? {
            Ok(who) => who,
            Err(refusal) => return Ok(refusal.to_json()),
        };

        let rows = self
            .storage
            .get_session_list(self.agent_id.clone(), None)
            .await
            .map_err(|e| VizierError(format!("could not list the conversations: {e}")))?;

        let wanted_channel = args.channel.as_deref().map(str::trim);
        let wanted_query = args
            .query
            .as_deref()
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .map(str::to_lowercase);

        let sessions: Vec<SessionEntry> = rows
            .into_iter()
            // Only the conversations a wake would accept: the wakeable channels (FR-014,
            // FR-015) and this person's own (FR-017).
            .filter(|row| is_wakeable(&row.channel))
            .filter(|row| access::belongs_to_person(&row.channel, &who.principal.username))
            .filter(|row| match wanted_channel {
                Some(slug) => row.channel.to_slug() == slug,
                None => true,
            })
            .filter(|row| match &wanted_query {
                Some(query) => row.title.to_lowercase().contains(query),
                None => true,
            })
            .map(|row| SessionEntry {
                channel: row.channel.to_slug(),
                topic_id: row.topic,
                title: row.title,
                is_thinking: row.is_thinking,
            })
            .collect();

        Ok(serde_json::json!({ "status": "ok", "sessions": sessions }))
    }
}

// ---------------------------------------------------------------------------------------------
// wake_session
// ---------------------------------------------------------------------------------------------

pub struct WakeSession {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
    pub transport: VizierTransport,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct WakeSessionArgs {
    /// The conversation to wake: "current" for the one this turn is in, or a channel slug as
    /// `list_sessions` returns it.
    pub target: String,

    /// The topic within the target channel, for the channels that have conversations of their
    /// own.
    #[serde(default)]
    pub topic_id: Option<String>,

    /// What to do in that conversation.
    pub instruction: String,

    /// Wait for the woken turn's answer and return it.
    #[serde(default)]
    pub wait: Option<bool>,
}

const WAKE_DESCRIPTION: &str = "\
start a turn in one of your own conversations, with an instruction. The work runs there, \
with that conversation's history in view, and the answer is saved into it. This is not \
posting a message: posting says something in a conversation at your cost, waking starts \
work there. Name the target explicitly — `target` is \"current\" or a channel from \
`list_sessions`, with its `topic_id` when the channel has one. Pass `wait: true` to have \
the answer come back in this call.";

#[async_trait::async_trait]
impl VizierTool for WakeSession {
    type Input = WakeSessionArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Sessions)
    }

    fn name() -> String {
        "wake_session".to_string()
    }

    fn description(&self) -> String {
        WAKE_DESCRIPTION.into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        access::may_wake(ctx)
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        if args.instruction.trim().is_empty() {
            return Ok(access::Refusal::BadRequest("the instruction is empty".into()).to_json());
        }

        let session = match resolve(&self.agent_id, &args, ctx) {
            Ok(session) => session,
            Err(refusal) => return Ok(refusal.to_json()),
        };

        // The person's own conversation, and nobody else's (FR-017).
        let who = match access::person(&self.storage, ctx).await? {
            Ok(who) => who,
            Err(refusal) => return Ok(refusal.to_json()),
        };
        if !access::belongs_to_person(&session.1, &who.principal.username) {
            let owner = session.1.web_user().unwrap_or_default().to_string();
            return Ok(access::Refusal::OtherPerson(owner).to_json());
        }

        // The hop limit (FR-020): the same constant the background work uses, and the only
        // guard a wake needs — with one agent, the only cycle is a conversation waking
        // another waking the first.
        if ctx.background_depth >= crate::schema::detached::MAX_BACKGROUND_DEPTH {
            return Ok(access::Refusal::TooDeep.to_json());
        }

        let request = VizierRequest {
            timestamp: crate::time::now(),
            user: self.agent_id.clone(),
            content: VizierRequestContent::Woken(args.instruction.clone()),
            metadata: serde_json::json!({}),
            background_depth: ctx.background_depth.saturating_add(1),
            // A woken turn keeps the waking turn's scope (spec 071 FR-030).
            tool_scope: ctx.tool_scope.as_deref().cloned(),
            ..Default::default()
        };

        // The reply stream is opened only when the caller waits for the answer.
        let (response_tx, response_rx) = if args.wait.unwrap_or(false) {
            match self.transport.reply_channel().await {
                Ok((tx, rx)) => (Some(tx), Some(rx)),
                Err(e) => {
                    return Err(VizierError(format!("could not open a reply stream: {e}")));
                }
            }
        } else {
            (None, None)
        };

        self.transport
            .send_request(session.clone(), request, response_tx)
            .await
            .map_err(|e| VizierError(format!("could not wake the conversation: {e}")))?;

        // The woken turn carries on whatever happens to this call: the reply stream is the
        // only thing this holds, and dropping it ends the wait, not the turn.
        let reply = match response_rx {
            Some(rx) => {
                let mut reply = None;
                while let Ok(response) = rx.recv_async().await {
                    if response.ends_turn() {
                        reply = match response.content {
                            VizierResponseContent::Message { content, .. } => Some(content),
                            _ => None,
                        };
                        break;
                    }
                }
                reply
            }
            None => None,
        };

        Ok(serde_json::json!({
            "status": "ok",
            "channel": session.1.to_slug(),
            "topic_id": session.2,
            "reply": reply,
        }))
    }
}

/// The conversation a call names, resolved against the calling agent (FR-005, FR-007).
fn resolve(
    agent_id: &AgentId,
    args: &WakeSessionArgs,
    ctx: &ToolContext,
) -> Result<VizierSession, access::Refusal> {
    let target = args.target.trim();
    if target.eq_ignore_ascii_case(CURRENT) {
        // The caller's own conversation. It is wakeable only when its channel is one that
        // may be woken: a scheduled run's own conversation is the machinery's, not the
        // agent's (FR-015).
        if !is_wakeable(&ctx.session.1) {
            return Err(access::Refusal::NotWakeable(format!(
                "this turn is running in the {} conversation, which the machinery owns",
                ctx.session.1.to_slug()
            )));
        }
        return Ok(ctx.session.clone());
    }

    let channel = wakeable_channel(target)?;
    if has_topics(&channel) && args.topic_id.as_deref().is_none_or(str::is_empty) {
        return Err(access::Refusal::BadRequest(format!(
            "{} has conversations of its own; give its topic_id",
            channel.to_slug()
        )));
    }
    Ok(VizierSession(
        agent_id.clone(),
        channel.clone(),
        topic_for(&channel, args.topic_id.clone()),
    ))
}
