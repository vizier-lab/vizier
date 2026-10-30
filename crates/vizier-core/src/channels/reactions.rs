//! The one ingest path for reactions, shared by every channel
//! (`specs/013-reaction-awareness/research.md` Decision 6).
//!
//! A channel only translates its native event into a [`ReactionChange`] and filters out its
//! own bots; resolving the target, checking it is an agent reply, storing the change and
//! broadcasting the new set all happen here. Nothing here branches on the channel, and no
//! reaction ever starts a turn.

use anyhow::Result;

use crate::{
    schema::{SessionHistoryContent, VizierResponseContent},
    storage::{
        VizierStorage,
        reaction::{Platform, ReactionStorage, Reactor},
    },
    transport::{SessionEvent, SessionFrame, VizierTransport},
};

/// Longest emoji key accepted, in bytes. Bounds what is stored and what the digest renders.
const MAX_EMOJI_BYTES: usize = 64;

pub enum ReactionTarget {
    /// A history uid, as the WebUI addresses messages.
    History(String),
    /// A message the agent posted on a platform, resolved through `platform_message_link`.
    Platform {
        agent_id: String,
        platform: Platform,
        chat_id: String,
        message_id: String,
    },
}

pub enum ReactionKind {
    Add(String),
    Remove(String),
    /// A moderator removed every reaction of one emoji.
    ClearEmoji(String),
    /// A moderator removed every reaction.
    ClearAll,
}

pub struct ReactionChange {
    pub target: ReactionTarget,
    /// Ignored for `ClearEmoji` and `ClearAll`.
    pub reactor: Reactor,
    pub kind: ReactionKind,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    /// Dropped by rule, not by failure: the reason is user-presentable.
    Ignored(&'static str),
}

pub async fn apply(
    storage: &VizierStorage,
    transport: &VizierTransport,
    change: ReactionChange,
) -> Result<ApplyOutcome> {
    let entry = match &change.target {
        ReactionTarget::History(uid) => storage.get_history_entry(uid).await?,
        ReactionTarget::Platform {
            agent_id,
            platform,
            chat_id,
            message_id,
        } => {
            storage
                .find_linked_message(agent_id, *platform, chat_id, message_id)
                .await?
        }
    };
    let Some(entry) = entry else {
        return Ok(ignored("unknown message"));
    };

    let is_reply = matches!(
        &entry.content,
        SessionHistoryContent::Response(response) if matches!(
            response.content,
            VizierResponseContent::Message { .. } | VizierResponseContent::AudioReply(..)
        )
    );
    if !is_reply {
        return Ok(ignored("not an agent reply"));
    }

    let emoji = match &change.kind {
        ReactionKind::Add(emoji) | ReactionKind::Remove(emoji) | ReactionKind::ClearEmoji(emoji) => {
            Some(emoji.as_str())
        }
        ReactionKind::ClearAll => None,
    };
    if let Some(emoji) = emoji
        && (emoji.is_empty() || emoji.len() > MAX_EMOJI_BYTES)
    {
        return Ok(ignored("invalid emoji"));
    }

    let uid = entry.uid.as_str();
    match &change.kind {
        ReactionKind::Add(emoji) => storage.add_reaction(uid, &change.reactor, emoji).await?,
        ReactionKind::Remove(emoji) => {
            storage
                .remove_reaction(uid, &change.reactor.id, emoji)
                .await?
        }
        ReactionKind::ClearEmoji(emoji) => storage.clear_reactions(uid, Some(emoji)).await?,
        ReactionKind::ClearAll => storage.clear_reactions(uid, None).await?,
    }

    let reactions = storage.list_reactions(uid).await?;
    transport.publish_session_event(SessionEvent {
        session: entry.vizier_session,
        frame: SessionFrame::Reactions {
            history_uid: entry.uid,
            reactions,
        },
    });

    Ok(ApplyOutcome::Applied)
}

fn ignored(reason: &'static str) -> ApplyOutcome {
    tracing::debug!("reaction ignored: {reason}");
    ApplyOutcome::Ignored(reason)
}
