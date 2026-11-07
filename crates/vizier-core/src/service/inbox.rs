//! The inbox stream's decisions (`specs/051-realtime-notifications/contracts/inbox-stream.md`):
//! what a person is told when they connect, and which bus events they are told about.
//!
//! The stream is a thin adapter (WebSocket or desktop IPC); everything that decides who may
//! know what lives here, next to the REST operations it must agree with
//! ([`super::notifications`], [`super::setup_state`]). A frame is checked against the person's
//! **current** permissions each time it is delivered, never against the ones they connected with,
//! so a revoked agent goes quiet from the next event.

use serde::Serialize;

use crate::{
    storage::workspace::WorkspaceStorage,
    access::can_view_agent,
    auth::{AuthMethod, Principal},
    schema::{AgentConfig, NotificationCounts, NotificationItem},
    service::{ServiceError, Services, setup_state},
    setup::SetupState,
    storage::{agent::AgentStorage, notification::NotificationStorage},
    transport::InboxEvent,
};

/// One message to a client. `type` is the discriminator on the wire; a client ignores types it
/// does not know, so frames can be added later.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InboxFrame {
    /// First frame on every connection: replaces everything the client holds.
    Hello {
        counts: NotificationCounts,
        /// The highest notification id this person may see, or `0`.
        latest_id: i64,
        setups: Vec<SetupState>,
    },
    /// A row this person may see was created. Always unread.
    Notification {
        item: NotificationItem,
        counts: NotificationCounts,
    },
    /// This person's read state changed on any client. No `id` means mark-all.
    Read {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<i64>,
        read: bool,
        counts: NotificationCounts,
    },
    /// One agent's setup changed. `owner_id` is never sent.
    Setup { state: SetupState },
    SetupCleared { agent_id: String },
    /// What the client holds may be out of date: reload the first page and the counts.
    Resync,
}

impl InboxFrame {
    /// Whether the frame carries counts that the connection has to fill in.
    pub fn needs_counts(&self) -> bool {
        matches!(self, Self::Notification { .. } | Self::Read { .. })
    }

    pub fn set_counts(&mut self, fresh: NotificationCounts) {
        if let Self::Notification { counts, .. } | Self::Read { counts, .. } = self {
            *counts = fresh;
        }
    }
}

/// What to do with a batch of bus events for one connection.
#[derive(Debug, Clone, PartialEq)]
pub enum Delivery {
    /// Send these frames, in order. Possibly none: nothing in the batch was for this person.
    Frames(Vec<InboxFrame>),
    /// The person no longer resolves (deleted, or their role is gone): end the connection.
    Close,
}

/// The pure rule: may `principal` be told about `event`, and as what? `agents` is every stored
/// agent, because whether a setup belongs to an agent not yet stored matters.
pub fn judge(principal: &Principal, agents: &[(String, AgentConfig)], event: &InboxEvent) -> Option<InboxFrame> {
    judge_with(principal, agents, &[], event)
}

/// [`judge`], also knowing the Workspaces the person is in: a Workspace message is for every
/// current member but its author, whether or not they can view the agent that wrote it
/// (`specs/053-workspace-channel` research R20).
pub fn judge_with(
    principal: &Principal,
    agents: &[(String, AgentConfig)],
    workspaces: &[String],
    event: &InboxEvent,
) -> Option<InboxFrame> {
    let can_view = |agent_id: &str| {
        agents
            .iter()
            .any(|(id, config)| id == agent_id && can_view_agent(principal, config))
    };
    let mine = |user: &str| user == principal.user_id;
    match event {
        InboxEvent::Created { item, audience, source_session, author } => {
            let in_workspace = item
                .agent_id
                .strip_prefix(crate::schema::WORKSPACE_SCOPE_PREFIX)
                .is_some_and(|id| workspaces.iter().any(|w| w == id));
            let visible = (can_view(&item.agent_id) || in_workspace)
                && audience.as_deref().is_none_or(mine)
                && author.as_deref() != Some(principal.user_id.as_str());
            visible.then(|| {
                // The item's own session is not serialized, so it came separately.
                let mut item = item.clone();
                item.source_session = source_session.clone();
                InboxFrame::Notification { item, counts: NotificationCounts::default() }
            })
        }
        InboxEvent::Read { user_id, id, read } => mine(user_id).then(|| InboxFrame::Read {
            id: *id,
            read: *read,
            counts: NotificationCounts::default(),
        }),
        InboxEvent::Setup { state, owner_id } => {
            setup_state::can_see_setup(principal, agents, &state.agent_id, owner_id.as_deref())
                .then(|| InboxFrame::Setup { state: state.clone() })
        }
        // The owner is not known any more, so a setup that was never stored is announced
        // cleared only to those who may see every agent; the client drops a finished entry on
        // its own after a few seconds either way.
        InboxEvent::SetupCleared { agent_id } => {
            setup_state::can_see_setup(principal, agents, agent_id, None)
                .then(|| InboxFrame::SetupCleared { agent_id: agent_id.clone() })
        }
        InboxEvent::Stale { user, agent_id } => {
            let for_me = user.as_deref().is_none_or(mine);
            let in_view = agent_id.as_deref().is_none_or(can_view);
            (for_me && in_view).then_some(InboxFrame::Resync)
        }
    }
}

/// Decide what `user_id` is told about the `events` that have arrived, re-reading who they are
/// first: their role and permissions as they are now, not as they were when they connected.
///
/// Events that queued up together are one batch, so the person is resolved and the agents
/// listed once, and the counts the frames carry are read once and are the state after the
/// whole batch.
pub async fn deliver(
    services: &Services,
    user_id: &str,
    method: AuthMethod,
    events: &[InboxEvent],
) -> Delivery {
    let principal = match services.resolve(user_id, method).await {
        Ok(principal) => principal,
        Err(ServiceError::Unauthorized(_)) => return Delivery::Close,
        Err(e) => {
            tracing::warn!("could not re-check who an inbox stream belongs to: {e}");
            return Delivery::Frames(vec![]);
        }
    };
    let agents = match services.deps().storage.list_agents().await {
        Ok(agents) => agents,
        Err(e) => {
            tracing::warn!("could not list agents for an inbox event: {e}");
            return Delivery::Frames(vec![]);
        }
    };
    let workspaces: Vec<String> = services
        .deps()
        .storage
        .workspaces_of_person(&principal.username)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(workspace, _)| workspace.id)
        .collect();
    let mut frames: Vec<InboxFrame> = events
        .iter()
        .filter_map(|event| judge_with(&principal, &agents, &workspaces, event))
        .collect();
    for frame in frames.iter_mut() {
        if let InboxFrame::Notification { item, .. } = frame {
            super::notifications::fill_titles(services, &principal, std::slice::from_mut(item)).await;
        }
    }
    if frames.iter().any(InboxFrame::needs_counts) {
        match counts(services, &principal).await {
            Ok(fresh) => frames.iter_mut().for_each(|frame| frame.set_counts(fresh)),
            // Better a frame with stale counts than a dropped notification: the next
            // `hello` or `resync` corrects the number.
            Err(e) => tracing::warn!("could not read the counts for an inbox frame: {e}"),
        }
    }
    Delivery::Frames(frames)
}

/// This person's counts over the agents they may view.
pub async fn counts(services: &Services, principal: &Principal) -> Result<NotificationCounts, ServiceError> {
    let agents = visible_ids(services, principal).await?;
    services
        .deps()
        .storage
        .notification_counts(&principal.user_id, &agents)
        .await
        .map_err(|e| ServiceError::Internal(e.to_string()))
}

/// What a connection sends first: the counts, the high-water mark and the setups in progress.
pub async fn hello(services: &Services, principal: &Principal) -> Result<InboxFrame, ServiceError> {
    let agents = visible_ids(services, principal).await?;
    let storage = &services.deps().storage;
    let internal = |e: anyhow::Error| ServiceError::Internal(e.to_string());
    let counts = storage
        .notification_counts(&principal.user_id, &agents)
        .await
        .map_err(internal)?;
    let latest_id = storage
        .latest_notification_id(&principal.user_id, &agents)
        .await
        .map_err(internal)?;
    let setups = setup_state::states(services, principal).await?.setups;
    Ok(InboxFrame::Hello { counts, latest_id, setups })
}

/// [`hello`] for a person identified by id and how they authenticated, resolved just now. This
/// is what a connection sends again after it fell behind. `Unauthorized` means they are gone.
pub async fn hello_for(services: &Services, user_id: &str, method: AuthMethod) -> Result<InboxFrame, ServiceError> {
    let principal = services.resolve(user_id, method).await?;
    hello(services, &principal).await
}

async fn visible_ids(services: &Services, principal: &Principal) -> Result<Vec<String>, ServiceError> {
    let mut ids: Vec<String> = super::overview::visible_agents(services, principal)
        .await?
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    ids.extend(super::notifications::workspace_scopes(services, principal).await);
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        auth::AuthMethod,
        schema::{NotificationCategory, NotificationTone},
        setup::SetupPhase,
        storage::user::Role,
        tests_support::agent_config,
    };

    fn principal(id: &str, system: bool, permissions: &[&str]) -> Principal {
        Principal {
            user_id: id.into(),
            username: id.into(),
            role: Role {
                role_id: "r".into(),
                name: "role".into(),
                permissions: vec![],
                is_system: system,
                created_at: crate::time::now(),
            },
            permissions: permissions.iter().map(|p| p.to_string()).collect(),
            method: AuthMethod::Jwt,
        }
    }

    fn agents() -> Vec<(String, AgentConfig)> {
        let mut mine = agent_config("mine");
        mine.owner_id = Some("ana".into());
        let mut theirs = agent_config("theirs");
        theirs.owner_id = Some("bob".into());
        vec![("mine".into(), mine), ("theirs".into(), theirs)]
    }

    fn created(agent_id: &str, audience: Option<&str>) -> InboxEvent {
        InboxEvent::Created {
            item: NotificationItem {
                id: 1,
                at: crate::time::now(),
                agent_id: agent_id.into(),
                category: NotificationCategory::TaskRun,
                tone: NotificationTone::Default,
                text: "Task answered.".into(),
                href: "/x".into(),
                read: false,
                title: None,
                excerpt: None,
                source_session: None,
            },
            audience: audience.map(str::to_string),
            source_session: None,
            author: None,
        }
    }

    /// A Workspace message by `author` (a user id or an agent id).
    fn workspace_message(workspace: &str, author: &str) -> InboxEvent {
        let InboxEvent::Created { item, audience, source_session, .. } = created(&crate::schema::workspace_scope(workspace), None) else {
            unreachable!()
        };
        InboxEvent::Created { item, audience, source_session, author: Some(author.into()) }
    }

    fn setup(agent_id: &str) -> SetupState {
        SetupState {
            agent_id: agent_id.into(),
            name: agent_id.into(),
            phase: SetupPhase::Downloading,
            message: "model".into(),
            error: None,
            downloads: vec![],
            owner_id: None,
        }
    }

    #[test]
    fn a_workspace_message_is_for_every_member_but_its_author() {
        let ana = principal("ana", false, &["owned_agents:view"]);
        let in_room = ["w1".to_string()];
        // Visible to a member even though they cannot view the agent that wrote it.
        assert!(judge_with(&ana, &agents(), &in_room, &workspace_message("w1", "theirs")).is_some());
        // Not to a person who is not in that Workspace.
        assert!(judge_with(&ana, &agents(), &["w2".to_string()], &workspace_message("w1", "theirs")).is_none());
        assert!(judge(&ana, &agents(), &workspace_message("w1", "theirs")).is_none());
        // Never to its author.
        assert!(judge_with(&ana, &agents(), &in_room, &workspace_message("w1", "ana")).is_none());
    }

    #[test]
    fn a_row_reaches_those_who_can_view_its_agent_and_nobody_else() {
        let ana = principal("ana", false, &[]);
        let admin = principal("root", true, &[]);
        assert!(judge(&ana, &agents(), &created("mine", None)).is_some());
        assert!(judge(&ana, &agents(), &created("theirs", None)).is_none(), "a hidden agent");
        assert!(judge(&admin, &agents(), &created("theirs", None)).is_some());
        assert!(judge(&ana, &agents(), &created("gone", None)).is_none(), "an agent not stored");
    }

    #[test]
    fn an_addressed_row_reaches_its_person_only() {
        let admin = principal("root", true, &[]);
        let ana = principal("ana", false, &[]);
        assert!(judge(&ana, &agents(), &created("mine", Some("ana"))).is_some());
        assert!(
            judge(&admin, &agents(), &created("mine", Some("ana"))).is_none(),
            "even someone who can view the agent does not see another person's conversation"
        );
    }

    #[test]
    fn a_read_change_reaches_the_person_who_made_it_only() {
        let ana = principal("ana", false, &[]);
        let bob = principal("bob", false, &[]);
        let event = InboxEvent::Read { user_id: "ana".into(), id: Some(7), read: true };
        assert!(matches!(
            judge(&ana, &agents(), &event),
            Some(InboxFrame::Read { id: Some(7), read: true, .. })
        ));
        assert!(judge(&bob, &agents(), &event).is_none());
        let all = InboxEvent::Read { user_id: "ana".into(), id: None, read: true };
        assert!(matches!(judge(&ana, &agents(), &all), Some(InboxFrame::Read { id: None, .. })));
    }

    #[test]
    fn a_setup_of_a_stored_agent_follows_who_may_view_it() {
        let ana = principal("ana", false, &[]);
        let event = InboxEvent::Setup { state: setup("mine"), owner_id: None };
        assert!(matches!(judge(&ana, &agents(), &event), Some(InboxFrame::Setup { .. })));
        let hidden = InboxEvent::Setup { state: setup("theirs"), owner_id: None };
        assert!(judge(&ana, &agents(), &hidden).is_none());
    }

    #[test]
    fn a_setup_of_an_agent_still_being_created_reaches_its_owner_and_not_a_stranger() {
        let owner = principal("ana", false, &[]);
        let stranger = principal("bob", false, &[]);
        let event = InboxEvent::Setup { state: setup("scout"), owner_id: Some("ana".into()) };
        assert!(judge(&owner, &agents(), &event).is_some());
        assert!(judge(&stranger, &agents(), &event).is_none());
    }

    #[test]
    fn a_stale_notice_is_scoped_to_a_person_or_the_viewers_of_an_agent() {
        let ana = principal("ana", false, &[]);
        let for_ana = InboxEvent::Stale { user: Some("ana".into()), agent_id: None };
        let for_bob = InboxEvent::Stale { user: Some("bob".into()), agent_id: None };
        assert_eq!(judge(&ana, &agents(), &for_ana), Some(InboxFrame::Resync));
        assert!(judge(&ana, &agents(), &for_bob).is_none());
        let mine = InboxEvent::Stale { user: None, agent_id: Some("mine".into()) };
        let theirs = InboxEvent::Stale { user: None, agent_id: Some("theirs".into()) };
        assert!(judge(&ana, &agents(), &mine).is_some());
        assert!(judge(&ana, &agents(), &theirs).is_none());
        assert!(judge(&ana, &agents(), &InboxEvent::Stale { user: None, agent_id: None }).is_some());
    }

    #[test]
    fn the_owner_never_travels_to_a_client() {
        let frame = InboxFrame::Setup { state: setup("scout") };
        let json = serde_json::to_value(&frame).unwrap();
        assert_eq!(json["type"], "setup");
        assert!(json["state"].get("owner_id").is_none());
        let resync = serde_json::to_value(InboxFrame::Resync).unwrap();
        assert_eq!(resync, serde_json::json!({ "type": "resync" }));
        let read = serde_json::to_value(InboxFrame::Read {
            id: None,
            read: true,
            counts: NotificationCounts::default(),
        })
        .unwrap();
        assert!(read.get("id").is_none(), "mark-all has no id");
    }

    #[test]
    fn counts_are_filled_only_into_frames_that_carry_them() {
        let mut frame = InboxFrame::Read { id: Some(1), read: true, counts: NotificationCounts::default() };
        assert!(frame.needs_counts());
        frame.set_counts(NotificationCounts { all: 3, unread: 2, read: 1 });
        assert!(matches!(&frame, InboxFrame::Read { counts, .. } if counts.unread == 2));
        let mut resync = InboxFrame::Resync;
        assert!(!resync.needs_counts());
        resync.set_counts(NotificationCounts { all: 1, unread: 1, read: 0 });
        assert_eq!(resync, InboxFrame::Resync);
    }
}
