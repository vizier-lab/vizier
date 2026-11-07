//! The Workspace channel (`specs/053-workspace-channel/`): a shared space with people, agents
//! and topics, where agents read silently and wake only when mentioned.
//!
//! Each agent keeps its own record of a topic: one session per agent per topic on a
//! [`VizierChannelId::Workspace`] channel, holding its own turns plus what was shown of the
//! others. `workspace_entry` holds only the shown messages, in one order, for everyone.

pub mod access;
pub mod catch_up;
pub mod commands;
pub mod context;
pub mod mentions;
pub mod names;

use crate::{
    dependencies::VizierDependencies,
    schema::{
        EntryKind, EntryMention, MemberKind, MentionState, NewEntry, VizierAttachment,
        VizierAttachmentContent, VizierChannelId, VizierRequest, VizierRequestContent, VizierSession, Workspace, WorkspaceEntry,
    },
    storage::{VizierStorage, turn::TurnStorage, workspace::WorkspaceStorage},
    transport::{VizierTransport, WorkspaceEvent},
};

/// What the Workspace engine needs: the storage and the transport. A gateway builds it from the
/// dependencies; a running turn builds it from its own handles, because the agent loop saves
/// the messages and so has to announce them and holds no `VizierDependencies`.
#[derive(Clone)]
pub struct Hub {
    pub storage: VizierStorage,
    pub transport: VizierTransport,
}

impl Hub {
    pub fn new(storage: VizierStorage, transport: VizierTransport) -> Self {
        Self { storage, transport }
    }
}

impl From<&VizierDependencies> for Hub {
    fn from(deps: &VizierDependencies) -> Self {
        Self { storage: (*deps.storage).clone(), transport: deps.transport.clone() }
    }
}

/// The slug prefix of a Workspace session's channel (`workspace__{id}`). Every prefix check that
/// decides who may see a conversation must account for it (research R8).
pub const PREFIX: &str = "workspace__";

/// The longest message a person can post, in characters.
pub const MAX_MESSAGE_CHARS: usize = 32_000;

/// The session an agent keeps for one topic.
pub fn session_of(agent_id: &str, workspace_id: &str, topic_id: &str) -> VizierSession {
    VizierSession(
        agent_id.to_string(),
        VizierChannelId::Workspace(workspace_id.to_string()),
        Some(topic_id.to_string()),
    )
}

/// Why a message was not written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostError {
    Empty,
    TooLong,
    /// A command a Workspace does not offer, with the reason to show.
    Refused(&'static str),
    Internal(String),
}

impl From<anyhow::Error> for PostError {
    fn from(e: anyhow::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

fn now_ms() -> i64 {
    crate::time::now().timestamp_millis()
}

/// Writes an entry: one transaction for the entry and its mentions, then the announcement and the
/// wake-ups. Used for people's messages and for what tools write; an agent's own text is written
/// by the history save and only announced (see [`after_save`]).
pub async fn write_entry(
    hub: &Hub,
    draft: NewEntry,
) -> Result<WorkspaceEntry, PostError> {
    if draft.kind != EntryKind::Note {
        if draft.body.trim().is_empty() && draft.files.is_empty() {
            return Err(PostError::Empty);
        }
        if draft.body.chars().count() > MAX_MESSAGE_CHARS {
            return Err(PostError::TooLong);
        }
    }
    let members = hub.storage.list_members(&draft.workspace_id).await?;
    let handles: Vec<(&str, MemberKind)> = members
        .iter()
        // Naming yourself wakes nobody.
        .filter(|m| !(m.kind == draft.author_kind && m.handle == draft.author))
        .map(|m| (m.handle.as_str(), m.kind))
        .collect();
    let names: Vec<&str> = handles.iter().map(|(h, _)| *h).collect();

    // A Workspace offers neither side chats nor clearing files.
    if draft.kind == EntryKind::Person {
        let agents: Vec<&str> =
            handles.iter().filter(|(_, k)| *k == MemberKind::Agent).map(|(h, _)| *h).collect();
        if let commands::Parsed::NotInWorkspace(reason) = commands::parse(&draft.body, &agents) {
            return Err(PostError::Refused(reason));
        }
    }

    let mentions: Vec<(MemberKind, String, MentionState)> = if draft.kind == EntryKind::Note {
        vec![]
    } else {
        mentions::parse(&draft.body, &names)
            .into_iter()
            .filter_map(|m| {
                let kind = handles.iter().find(|(h, _)| *h == m.handle)?.1;
                let state = match kind {
                    MemberKind::Agent => MentionState::Pending,
                    MemberKind::Person => MentionState::Resolved,
                };
                Some((kind, m.handle, state))
            })
            .collect()
    };
    let entry = hub.storage.insert_entry(&draft, &mentions).await?;
    announce_and_dispatch(hub, &entry).await;
    Ok(entry)
}

/// Writes a line the system adds, such as a command someone ran. It is shown to members and
/// never copied into an agent's record.
pub async fn write_note(
    hub: &Hub,
    workspace_id: &str,
    topic_id: &str,
    text: String,
) -> Result<WorkspaceEntry, PostError> {
    write_entry(
        hub,
        NewEntry {
            workspace_id: workspace_id.to_string(),
            topic_id: topic_id.to_string(),
            kind: EntryKind::Note,
            author_kind: MemberKind::Person,
            author: "system".into(),
            author_name: "system".into(),
            body: text,
            files: vec![],
            history_uid: None,
            source_session: None,
            origin: None,
            is_final: false,
            activated_by_seq: None,
            at: now_ms(),
        },
    )
    .await
}

/// One notification row for a message: seen by every current member but its author
/// (`specs/053-workspace-channel` research R20). A note makes none, and `event_key` makes a second
/// call for the same entry a no-op, so the startup sweep can repeat it safely.
pub async fn notify(hub: &Hub, entry: &WorkspaceEntry) {
    use crate::{
        notifications::{Notifications, excerpt},
        schema::{NotificationCategory, NotificationDraft, NotificationTone, workspace_scope},
        storage::user::UserStorage,
    };
    if entry.kind == EntryKind::Note {
        return;
    }
    let (Ok(Some(workspace)), Ok(Some(topic))) = (
        hub.storage.get_workspace(&entry.workspace_id).await,
        hub.storage.get_topic(&entry.workspace_id, &entry.topic_id).await,
    ) else {
        return;
    };
    // The author is a user id for a person (what a viewer is compared with), an agent id otherwise.
    let author = match entry.author_kind {
        MemberKind::Person => hub
            .storage
            .get_user(&entry.author)
            .await
            .ok()
            .flatten()
            .map_or_else(|| entry.author.clone(), |u| u.user_id),
        MemberKind::Agent => entry.author.clone(),
    };
    let draft = NotificationDraft {
        event_key: format!("workspace:{}", entry.seq),
        agent_id: workspace_scope(&entry.workspace_id),
        audience_user_id: None,
        category: NotificationCategory::WorkspaceMessage,
        tone: NotificationTone::Default,
        summary: format!("{} in {} #{}", entry.author_name, workspace.name, topic.name),
        href: format!("/workspaces/{}/{}", workspace.id, topic.id),
        created_at: crate::time::from_millis(entry.at),
        source_session: None,
        source_title: Some(format!("{} #{}", workspace.name, topic.name)),
        excerpt: excerpt::from_markdown(&entry.body),
        workspace_id: Some(entry.workspace_id.clone()),
        author: Some(author),
    };
    let notifications = Notifications::new(std::sync::Arc::new(hub.storage.clone()));
    notifications.install_transport(hub.transport.clone());
    notifications.record_all(vec![draft]).await;
}

/// Tells everyone watching about an entry, notifies the other members, then wakes the agents it
/// mentions.
pub async fn announce_and_dispatch(hub: &Hub, entry: &WorkspaceEntry) {
    notify(hub, entry).await;
    let mentions = hub
        .storage
        .mentions_of(&[entry.seq])
        .await
        .unwrap_or_default();
    hub.transport.publish_workspace_event(WorkspaceEvent::Entry {
        entry: entry.clone(),
        mentions,
    });
    dispatch_mentions(hub, entry).await;
}

/// Called after an agent's text was saved to its history, which wrote the entry in the same
/// transaction: announce it and wake who it names.
pub async fn after_save(hub: &Hub, history_uid: &str) {
    match hub.storage.entry_by_history_uid(history_uid).await {
        Ok(Some(entry)) => announce_and_dispatch(hub, &entry).await,
        Ok(None) => {}
        Err(e) => tracing::warn!("could not read the Workspace entry of a saved message: {e}"),
    }
}

/// Wakes each agent an entry names whose mention is still pending.
///
/// A mention written by a turn that Stop all already covered is dropped: with no chain limit, the
/// stop is the only brake, and a turn finishing a moment later must not restart the chain.
pub async fn dispatch_mentions(hub: &Hub, entry: &WorkspaceEntry) {
    let pending: Vec<EntryMention> = match hub.storage.mentions_of(&[entry.seq]).await {
        Ok(all) => all
            .into_iter()
            .filter(|m| m.target_kind == MemberKind::Agent && m.state == MentionState::Pending)
            .collect(),
        Err(e) => {
            tracing::warn!("could not read the mentions of entry {}: {e}", entry.seq);
            return;
        }
    };
    if pending.is_empty() {
        return;
    }
    let workspace = match hub.storage.get_workspace(&entry.workspace_id).await {
        Ok(Some(w)) => w,
        _ => return,
    };
    let members = hub.storage.list_members(&entry.workspace_id).await.unwrap_or_default();
    let agents: Vec<&str> = members
        .iter()
        .filter(|m| m.kind == MemberKind::Agent)
        .map(|m| m.handle.as_str())
        .collect();

    let mut changed = false;
    for mention in pending {
        let stopped = entry.kind == EntryKind::Agent
            && workspace.stopped_at_seq > 0
            && entry.activated_by_seq.is_some_and(|a| a <= workspace.stopped_at_seq);
        let state = if stopped {
            MentionState::Dropped
        } else if !agents.iter().any(|a| *a == mention.target) {
            // Removed since the entry was written.
            MentionState::Unavailable
        } else {
            wake(hub, &workspace, entry, &mention.target, &agents).await
        };
        let _ = hub
            .storage
            .set_mention_state(entry.seq, &mention.target, state, now_ms())
            .await;
        changed = true;
    }
    if changed {
        publish_mentions(hub, entry).await;
    }
}

async fn publish_mentions(hub: &Hub, entry: &WorkspaceEntry) {
    if let Ok(mentions) = hub.storage.mentions_of(&[entry.seq]).await {
        hub.transport.publish_workspace_event(WorkspaceEvent::Mentions {
            workspace_id: entry.workspace_id.clone(),
            topic_id: entry.topic_id.clone(),
            seq: entry.seq,
            mentions,
        });
    }
}

/// Sends one agent the request an activation is: an ordinary `Chat` through the agent seam, so
/// durable turns, resume, skills, queuing and abort all work unchanged.
async fn wake(
    hub: &Hub,
    workspace: &Workspace,
    entry: &WorkspaceEntry,
    agent_id: &str,
    agents: &[&str],
) -> MentionState {
    let session = session_of(agent_id, &workspace.id, &entry.topic_id);
    let (text, plain) = commands::request_for(&entry.body, agent_id, agents);
    let request = VizierRequest {
        timestamp: crate::time::now(),
        user: entry.author.clone(),
        content: VizierRequestContent::Chat(text),
        metadata: serde_json::json!({
            "workspace_entry": entry.seq,
            "workspace_plain": plain,
        }),
        // The files the message came with, as in a direct chat.
        attachments: entry
            .files
            .iter()
            .map(|f| VizierAttachment {
                filename: f.name.clone(),
                content: VizierAttachmentContent::Local(format!("/api/v1/files/{}", f.file_id)),
            })
            .collect(),
        ..Default::default()
    };
    // Its frames reach whoever watches the topic, as a woken background report's do.
    let reply = hub.transport.broadcast_sender(session.clone()).await.ok();
    match hub.transport.send_request(session, request, reply).await {
        Ok(()) => MentionState::Woken,
        Err(e) => {
            tracing::warn!(agent = agent_id, "could not wake an agent in a Workspace: {e}");
            MentionState::Unavailable
        }
    }
}

/// At startup: delivers the mentions that were saved and never sent, because the process stopped
/// between the save and the dispatch.
pub async fn redispatch_pending(hub: &Hub) {
    let pending = match hub.storage.pending_mentions().await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("could not read pending Workspace mentions: {e}");
            return;
        }
    };
    let mut seen = std::collections::BTreeSet::new();
    for mention in pending {
        if !seen.insert(mention.entry_seq) {
            continue;
        }
        if let Ok(Some(entry)) = hub.storage.get_entry(mention.entry_seq).await {
            // A row lost between the save and the stop is made now; one already there is kept.
            notify(hub, &entry).await;
            dispatch_mentions(hub, &entry).await;
        }
    }
}

/// What stopping did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stopped {
    /// Turns that were running or queued and were aborted.
    pub stopped: usize,
    /// Mentions that were waiting and will not be delivered.
    pub dropped: usize,
}

async fn busy_sessions(hub: &Hub, workspace_id: &str) -> Vec<VizierSession> {
    hub.storage
        .sessions_with_pending_turns()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|s| matches!(&s.1, VizierChannelId::Workspace(id) if id == workspace_id))
        .collect()
}

/// Aborts the turn running in a session: the request that aborts a turn today, which is also what
/// `AgentsClient::abort` sends.
pub(crate) async fn abort(hub: &Hub, session: &VizierSession) {
    let request = VizierRequest {
        timestamp: crate::time::now(),
        user: "system".into(),
        content: VizierRequestContent::Command("abort".into()),
        metadata: serde_json::json!({}),
        ..Default::default()
    };
    let _ = hub.transport.send_request(session.clone(), request, None).await;
}

/// Stop all (research R7): records the head so a turn finishing now cannot restart a chain,
/// drops the mentions still waiting, and aborts every member agent's turn in every topic.
pub async fn stop_all(hub: &Hub, workspace_id: &str) -> anyhow::Result<Stopped> {
    let (dropped, _) = hub.storage.stop_all(workspace_id, now_ms()).await?;
    let busy = busy_sessions(hub, workspace_id).await;
    for session in &busy {
        abort(hub, session).await;
    }
    Ok(Stopped { stopped: busy.len(), dropped })
}

/// Sends an agent a command (`checkpoint` or `lobotomy`) for its record of one topic. It runs on
/// the agent's own session and touches nobody else's context.
pub async fn send_command(hub: &Hub, workspace_id: &str, topic_id: &str, agent_id: &str, command: &str) {
    let session = session_of(agent_id, workspace_id, topic_id);
    let request = VizierRequest {
        timestamp: crate::time::now(),
        user: "system".into(),
        content: VizierRequestContent::Command(command.to_string()),
        metadata: serde_json::json!({}),
        ..Default::default()
    };
    if let Err(e) = hub.transport.send_request(session, request, None).await {
        tracing::warn!(agent = agent_id, "could not send {command} to an agent in a Workspace: {e}");
    }
}

/// Stops one agent's turn in one topic.
pub async fn stop_agent(
    hub: &Hub,
    workspace_id: &str,
    topic_id: &str,
    agent_id: &str,
) -> anyhow::Result<Stopped> {
    let session = session_of(agent_id, workspace_id, topic_id);
    let busy = busy_sessions(hub, workspace_id).await.contains(&session);
    abort(hub, &session).await;
    Ok(Stopped { stopped: usize::from(busy), dropped: 0 })
}
