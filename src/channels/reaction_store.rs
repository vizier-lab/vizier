use anyhow::Result;

use crate::schema::{PlatformMessageId, ReactionEntry, VizierSession};
use crate::storage::history::HistoryStorage;
use crate::storage::VizierStorage;

pub async fn record_reaction(
    storage: &VizierStorage,
    session: &VizierSession,
    message_uid: &str,
    entry: ReactionEntry,
) -> Result<()> {
    let history = storage
        .list_session_history(session.clone(), None, None)
        .await?;

    let mut reactions = history
        .iter()
        .find(|h| h.uid == message_uid)
        .map(|h| h.reactions.clone())
        .unwrap_or_default();

    if let Some(pos) = reactions
        .iter()
        .position(|r| r.user_id == entry.user_id && r.emoji == entry.emoji)
    {
        reactions.remove(pos);
    } else {
        reactions.push(entry);
    }

    storage
        .update_history_reactions(message_uid.to_string(), session.clone(), reactions)
        .await?;

    Ok(())
}

pub async fn find_message_uid_by_platform_id(
    storage: &VizierStorage,
    session: &VizierSession,
    platform_id: &PlatformMessageId,
) -> Result<Option<String>> {
    let history = storage
        .list_session_history(session.clone(), None, None)
        .await?;
    for entry in history {
        if let crate::schema::SessionHistoryContent::Request(req) = &entry.content
            && let Some(ref pid) = req.platform_message_id
            && pid == platform_id
        {
            return Ok(Some(entry.uid));
        }
    }
    Ok(None)
}
