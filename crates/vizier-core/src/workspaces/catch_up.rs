//! Lazy silent reading (`specs/053-workspace-channel/research.md` R2): posting writes one entry
//! and sends nothing to agents that were not mentioned. When an agent is activated, and before a
//! dream cycle, it copies what it has not read into its own session, so its record of the topic
//! is what it would be if it had read every message as it arrived.

use crate::{
    schema::{CatchUp, VizierSession},
    storage::workspace::WorkspaceStorage,
    workspaces::Hub,
};

/// The most messages copied at once. Older unread ones are counted and left out, and the
/// agent is told how many, and where to read them.
pub const CAP: usize = 500;

/// Brings an agent up to the entry that activated it: copies the unread entries before
/// `activating_seq` and moves the position to it, since the activating entry arrives as the
/// turn's own request. `None` when the session is not a Workspace topic.
pub async fn for_activation(
    hub: &Hub,
    session: &VizierSession,
    activating_seq: i64,
) -> Option<anyhow::Result<CatchUp>> {
    let topic = session.2.as_deref()?;
    matches!(session.1, crate::schema::VizierChannelId::Workspace(_)).then_some(())?;
    Some(
        hub.storage
            .catch_up(session, topic, activating_seq, activating_seq, CAP)
            .await,
    )
}

/// Brings an idle agent up to everything written so far, so a dream cycle sees what it read.
pub async fn to_head(hub: &Hub, session: &VizierSession) -> Option<anyhow::Result<CatchUp>> {
    let topic = session.2.as_deref()?;
    let crate::schema::VizierChannelId::Workspace(workspace_id) = &session.1 else {
        return None;
    };
    let head = match hub.storage.head_seq(workspace_id).await {
        Ok(head) => head,
        Err(e) => return Some(Err(e)),
    };
    Some(hub.storage.catch_up(session, topic, head + 1, head, CAP).await)
}
