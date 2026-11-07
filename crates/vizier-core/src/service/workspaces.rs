//! Workspaces over the service layer (`specs/053-workspace-channel/contracts/http-api.md`).
//! Authorization lives here and in `workspaces::access`; handlers only translate.
//!
//! A person with no standing in a Workspace gets `NotFound` from every operation, the same as for
//! one that does not exist (FR-004).

use serde::{Deserialize, Serialize};

use super::{ServiceError, Services};
use crate::{
    access::{Caller, can_edit_agent},
    auth::Principal,
    schema::{
        EntryKind, EntryMention, MemberKind, NewEntry, Workspace, WorkspaceEntry, WorkspaceMember,
        WorkspaceRole, WorkspaceTopic,
    },
    storage::{
        agent::AgentStorage, notification::NotificationStorage, user::UserStorage,
        turn::TurnStorage, workspace::WorkspaceStorage,
    },
    transport::WorkspaceEvent,
    workspaces::{
        self, Hub, PostError,
        access::{self, Standing},
        commands::{self, Parsed},
        names::{self, NameError},
    },
};

/// The first topic of a new Workspace, unless the request names one.
pub const DEFAULT_TOPIC: &str = "general";

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TopicSummary {
    pub id: String,
    pub name: String,
    /// Messages from others the person has not read in this topic.
    pub unread: u64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct WorkspaceSummary {
    pub id: String,
    pub name: String,
    /// `owner`, `member`, or `admin` for someone who manages every Workspace without being in it.
    pub role: String,
    pub unread: u64,
    /// An agent is working in it.
    pub working: bool,
    pub topics: Vec<TopicSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct MemberView {
    /// `person` or `agent`.
    pub kind: String,
    pub handle: String,
    /// The agent's name, or the person's username.
    pub name: String,
    /// `owner` or `member`.
    pub role: String,
    /// An agent that was deleted after being added reads `false`.
    pub available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct WorkspaceDetail {
    #[serde(flatten)]
    pub workspace: WorkspaceSummary,
    pub members: Vec<MemberView>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateWorkspaceRequest {
    pub name: String,
    /// The first topic. Defaults to `general`.
    #[serde(default)]
    pub first_topic: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateTopicRequest {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AddMemberRequest {
    /// `person` or `agent`.
    pub kind: String,
    /// A username or an agent id.
    pub handle: String,
    /// `owner` or `member`; agents are always `member`.
    #[serde(default)]
    pub role: Option<String>,
}

fn internal(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Internal(e.to_string())
}

fn not_found() -> ServiceError {
    ServiceError::NotFound("workspace not found".into())
}

fn refusal(status: u16, code: &str, message: impl Into<String>) -> ServiceError {
    ServiceError::Invalid { status, message: message.into(), code: Some(code.into()) }
}

fn conflict(code: &str, message: impl Into<String>) -> ServiceError {
    ServiceError::Conflict { message: message.into(), code: Some(code.into()), data: None }
}

fn name_error(what: &str, e: NameError) -> ServiceError {
    let reason = match e {
        NameError::Empty => "is empty",
        NameError::TooLong => "is too long",
        NameError::BadCharacters => "has characters that are not allowed",
    };
    refusal(
        422,
        "invalid_name",
        format!("The {what} name {reason}. Use {}", NameError::RULE),
    )
}

fn role_word(standing: Standing) -> &'static str {
    match standing {
        Standing::Owner => "owner",
        Standing::Member => "member",
        Standing::Admin => "admin",
    }
}

/// The Workspace and the caller's standing in it. `NotFound` for a Workspace that is missing or
/// that the caller has no standing in.
pub async fn resolve(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
) -> Result<(Workspace, Standing), ServiceError> {
    let storage = &s.deps().storage;
    let workspace = storage
        .get_workspace(workspace_id)
        .await
        .map_err(internal)?
        .ok_or_else(not_found)?;
    let membership = storage
        .get_member(workspace_id, &who.username)
        .await
        .map_err(internal)?;
    let standing =
        access::standing(Caller::from(who), &who.username, membership.as_ref()).ok_or_else(not_found)?;
    Ok((workspace, standing))
}

/// As [`resolve`], and the caller must be able to manage the Workspace. A member who cannot gets
/// `Forbidden`: they know it exists.
pub async fn resolve_manager(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
) -> Result<(Workspace, Standing), ServiceError> {
    let (workspace, standing) = resolve(s, who, workspace_id).await?;
    if !standing.can_manage() {
        return Err(ServiceError::Forbidden("only an owner can do this".into()));
    }
    Ok((workspace, standing))
}

async fn summary(
    s: &Services,
    who: &Principal,
    workspace: &Workspace,
    standing: Standing,
) -> Result<WorkspaceSummary, ServiceError> {
    let storage = &s.deps().storage;
    let unread = storage
        .workspace_unread(&who.user_id, std::slice::from_ref(&workspace.id))
        .await
        .map_err(internal)?;
    let topics: Vec<TopicSummary> = storage
        .list_topics(&workspace.id)
        .await
        .map_err(internal)?
        .into_iter()
        .map(|t| {
            let count = unread.iter().find(|(_, topic, _)| *topic == t.id).map_or(0, |(_, _, n)| *n);
            TopicSummary { id: t.id, name: t.name, unread: count }
        })
        .collect();
    // An agent working in any topic of the Workspace.
    let working = {
        use crate::storage::turn::TurnStorage;
        storage
            .sessions_with_pending_turns()
            .await
            .unwrap_or_default()
            .iter()
            .any(|session| matches!(&session.1, crate::schema::VizierChannelId::Workspace(id) if *id == workspace.id))
    };
    Ok(WorkspaceSummary {
        id: workspace.id.clone(),
        name: workspace.name.clone(),
        role: role_word(standing).into(),
        unread: topics.iter().map(|t| t.unread).sum(),
        working,
        topics,
    })
}

/// The Workspaces the caller is a member of, for the rail.
pub async fn list(s: &Services, who: &Principal) -> Result<Vec<WorkspaceSummary>, ServiceError> {
    let mine = s
        .deps()
        .storage
        .workspaces_of_person(&who.username)
        .await
        .map_err(internal)?;
    let mut rows = Vec::with_capacity(mine.len());
    for (workspace, role) in mine {
        let standing = match role {
            WorkspaceRole::Owner => Standing::Owner,
            WorkspaceRole::Member => Standing::Member,
        };
        rows.push(summary(s, who, &workspace, standing).await?);
    }
    Ok(rows)
}

pub async fn create(
    s: &Services,
    who: &Principal,
    request: CreateWorkspaceRequest,
) -> Result<WorkspaceDetail, ServiceError> {
    if !access::can_create(Caller::from(who)) {
        return Err(ServiceError::Forbidden("Forbidden: insufficient permissions".into()));
    }
    let name = names::normalize(&request.name);
    names::validate(&name).map_err(|e| name_error("Workspace", e))?;
    let topic_name = request
        .first_topic
        .as_deref()
        .map(names::normalize)
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| DEFAULT_TOPIC.to_string());
    names::validate(&topic_name).map_err(|e| name_error("topic", e))?;

    let storage = &s.deps().storage;
    if storage.find_workspace_by_name(&name).await.map_err(internal)?.is_some() {
        return Err(conflict("name_in_use", "A Workspace with that name already exists"));
    }
    let now = crate::time::now().timestamp_millis();
    let workspace = Workspace {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        created_by: who.username.clone(),
        stopped_at_seq: 0,
        created_at: now,
        updated_at: now,
    };
    let topic = WorkspaceTopic {
        id: uuid::Uuid::new_v4().to_string(),
        workspace_id: workspace.id.clone(),
        name: topic_name,
        created_by: who.username.clone(),
        created_at: now,
    };
    let owner = WorkspaceMember {
        workspace_id: workspace.id.clone(),
        kind: MemberKind::Person,
        handle: who.username.clone(),
        role: WorkspaceRole::Owner,
        added_by: who.username.clone(),
        added_at: now,
    };
    storage
        .create_workspace(&workspace, &topic, &owner)
        .await
        .map_err(|e| {
            // Two creates with the same name at once: the unique key decides.
            if crate::storage::sql::is_constraint_violation(&e) {
                conflict("name_in_use", "A Workspace with that name already exists")
            } else {
                internal(e)
            }
        })?;
    detail(s, who, &workspace.id).await
}

pub async fn detail(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
) -> Result<WorkspaceDetail, ServiceError> {
    let (workspace, standing) = resolve(s, who, workspace_id).await?;
    let mut members = Vec::new();
    for m in s.deps().storage.list_members(&workspace.id).await.map_err(internal)? {
        members.push(member_view(s, &m).await?);
    }
    Ok(WorkspaceDetail { workspace: summary(s, who, &workspace, standing).await?, members })
}

async fn member_view(s: &Services, m: &WorkspaceMember) -> Result<MemberView, ServiceError> {
    let (name, available) = match m.kind {
        MemberKind::Person => (m.handle.clone(), true),
        MemberKind::Agent => match s.deps().storage.get_agent(&m.handle).await.map_err(internal)? {
            Some(agent) => (agent.name, true),
            None => (m.handle.clone(), false),
        },
    };
    Ok(MemberView {
        kind: m.kind.as_str().into(),
        handle: m.handle.clone(),
        name,
        role: m.role.as_str().into(),
        available,
    })
}

/// Marks a topic's messages read for the caller. Opening a topic does this.
pub async fn mark_topic_read(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
    topic_id: &str,
) -> Result<u64, ServiceError> {
    let (workspace, _) = resolve(s, who, workspace_id).await?;
    let topic = topic_of(s, &workspace, topic_id).await?;
    let marked = s
        .deps()
        .storage
        .mark_topic_read(&who.user_id, &workspace.id, &topic.id, crate::time::now())
        .await
        .map_err(internal)?;
    if marked > 0 {
        // Every open tab of theirs counts again.
        s.deps().transport.publish_inbox_event(crate::transport::InboxEvent::Stale {
            user: Some(who.user_id.clone()),
            agent_id: None,
        });
    }
    Ok(marked)
}

pub async fn create_topic(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
    request: CreateTopicRequest,
) -> Result<TopicSummary, ServiceError> {
    let (workspace, _) = resolve(s, who, workspace_id).await?;
    let name = names::normalize(&request.name);
    names::validate(&name).map_err(|e| name_error("topic", e))?;
    let topic = WorkspaceTopic {
        id: uuid::Uuid::new_v4().to_string(),
        workspace_id: workspace.id,
        name,
        created_by: who.username.clone(),
        created_at: crate::time::now().timestamp_millis(),
    };
    s.deps().storage.create_topic(&topic).await.map_err(|e| {
        if crate::storage::sql::is_constraint_violation(&e) {
            conflict("name_in_use", "A topic with that name already exists in this Workspace")
        } else {
            internal(e)
        }
    })?;
    // Member agents read a new topic from now: there is nothing to backfill.
    s.deps().storage.begin_reading_topic(&topic.workspace_id, &topic.id).await.map_err(internal)?;
    Ok(TopicSummary { id: topic.id, name: topic.name, unread: 0 })
}

pub async fn add_member(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
    request: AddMemberRequest,
) -> Result<MemberView, ServiceError> {
    let (workspace, _) = resolve_manager(s, who, workspace_id).await?;
    let storage = &s.deps().storage;
    let handle = request.handle.trim().to_string();
    if handle.is_empty() {
        return Err(ServiceError::invalid("a member needs a handle"));
    }
    let kind = MemberKind::parse(&request.kind)
        .ok_or_else(|| ServiceError::invalid("kind must be person or agent"))?;

    let role = match kind {
        MemberKind::Person => {
            storage
                .get_user(&handle)
                .await
                .map_err(internal)?
                .ok_or_else(|| ServiceError::NotFound(format!("no such person: {handle}")))?;
            match request.role.as_deref() {
                None => WorkspaceRole::Member,
                Some(text) => WorkspaceRole::parse(text)
                    .ok_or_else(|| ServiceError::invalid("role must be owner or member"))?,
            }
        }
        MemberKind::Agent => {
            let agent = storage
                .get_agent(&handle)
                .await
                .map_err(internal)?
                .ok_or_else(|| ServiceError::NotFound(format!("no such agent: {handle}")))?;
            // Adding an agent shares its use with every member (FR-005), so it takes the right
            // to edit it, not just to see it.
            if !can_edit_agent(who, &agent) {
                return Err(refusal(
                    403,
                    "cannot_edit_agent",
                    "Only someone who can edit an agent can add it to a Workspace",
                ));
            }
            WorkspaceRole::Member
        }
    };

    let member = WorkspaceMember {
        workspace_id: workspace.id,
        kind,
        handle,
        role,
        added_by: who.username.clone(),
        added_at: crate::time::now().timestamp_millis(),
    };
    storage.add_member(&member).await.map_err(|e| {
        if crate::storage::sql::is_constraint_violation(&e) {
            conflict("handle_in_use", "That name is already used by a member of this Workspace")
        } else {
            internal(e)
        }
    })?;
    if member.kind == MemberKind::Agent {
        // An agent joining reads every topic from now, with no backfill (FR-010).
        storage.begin_reading_agent(&member.workspace_id, &member.handle).await.map_err(internal)?;
    }
    s.deps().transport.publish_workspace_event(WorkspaceEvent::MemberChanged {
        workspace_id: member.workspace_id.clone(),
        removed: None,
    });
    member_view(s, &member).await
}

pub async fn remove_member(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
    kind: &str,
    handle: &str,
) -> Result<(), ServiceError> {
    let (workspace, standing) = resolve_manager(s, who, workspace_id).await?;
    let storage = &s.deps().storage;
    let kind = MemberKind::parse(kind).ok_or_else(|| ServiceError::invalid("kind must be person or agent"))?;
    let member = storage
        .get_member(&workspace.id, handle)
        .await
        .map_err(internal)?
        .filter(|m| m.kind == kind)
        .ok_or_else(|| ServiceError::NotFound("no such member".into()))?;

    if member.kind == MemberKind::Person && member.role == WorkspaceRole::Owner && !standing.can_orphan() {
        let owners = storage
            .list_members(&workspace.id)
            .await
            .map_err(internal)?
            .iter()
            .filter(|m| m.kind == MemberKind::Person && m.role == WorkspaceRole::Owner)
            .count();
        if owners <= 1 {
            return Err(conflict("last_owner", "A Workspace needs at least one owner"));
        }
    }
    storage
        .remove_member(&workspace.id, member.kind, &member.handle)
        .await
        .map_err(internal)?;
    if member.kind == MemberKind::Agent {
        // Its running and queued turns end now (SC-006); its record of each topic stays with it.
        let hub = Hub::from(s.deps());
        for topic in storage.list_topics(&workspace.id).await.map_err(internal)? {
            let session = workspaces::session_of(&member.handle, &workspace.id, &topic.id);
            let _ = storage.cancel_session_turns(&session).await;
            workspaces::abort(&hub, &session).await;
        }
    }
    s.deps().transport.publish_workspace_event(WorkspaceEvent::MemberChanged {
        workspace_id: workspace.id,
        removed: Some((member.kind, member.handle)),
    });
    Ok(())
}


#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RenameRequest {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SetRoleRequest {
    pub role: String,
}

fn name_conflict(e: anyhow::Error, what: &str) -> ServiceError {
    if crate::storage::sql::is_constraint_violation(&e) {
        conflict("name_in_use", format!("A {what} with that name already exists"))
    } else {
        internal(e)
    }
}

pub async fn rename(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
    request: RenameRequest,
) -> Result<WorkspaceSummary, ServiceError> {
    let (workspace, standing) = resolve_manager(s, who, workspace_id).await?;
    let name = names::normalize(&request.name);
    names::validate(&name).map_err(|e| name_error("Workspace", e))?;
    s.deps()
        .storage
        .rename_workspace(&workspace.id, &name, crate::time::now().timestamp_millis())
        .await
        .map_err(|e| name_conflict(e, "Workspace"))?;
    let renamed = Workspace { name, ..workspace };
    summary(s, who, &renamed, standing).await
}

/// Deletes a Workspace with its topics, messages and each member agent's record of it.
/// Turns are stopped first, so none writes into a conversation being removed.
pub async fn delete(s: &Services, who: &Principal, workspace_id: &str) -> Result<(), ServiceError> {
    let (workspace, _) = resolve_manager(s, who, workspace_id).await?;
    let storage = &s.deps().storage;
    let hub = Hub::from(s.deps());
    workspaces::stop_all(&hub, &workspace.id).await.map_err(internal)?;
    let topics = storage.list_topics(&workspace.id).await.map_err(internal)?;
    let agents: Vec<String> = storage
        .list_members(&workspace.id)
        .await
        .map_err(internal)?
        .into_iter()
        .filter(|m| m.kind == MemberKind::Agent)
        .map(|m| m.handle)
        .collect();
    for agent in &agents {
        for topic in &topics {
            forget_session(s, &workspaces::session_of(agent, &workspace.id, &topic.id)).await;
        }
    }
    let _ = storage.delete_workspace_notifications(&workspace.id, None).await;
    storage.delete_workspace(&workspace.id).await.map_err(internal)?;
    s.deps().transport.publish_workspace_event(WorkspaceEvent::Deleted { workspace_id: workspace.id });
    Ok(())
}

/// Removes one agent's conversation for a topic: files, history and session row.
async fn forget_session(s: &Services, session: &crate::schema::VizierSession) {
    use crate::storage::{session::SessionStorage, session_file::SessionFileStorage};
    let storage = &s.deps().storage;
    let _ = storage.cancel_session_turns(session).await;
    let _ = storage.delete_session_files(session).await;
    let _ = storage
        .delete_session(session.0.clone(), session.1.clone(), session.2.clone().unwrap_or_default())
        .await;
}

pub async fn set_role(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
    handle: &str,
    request: SetRoleRequest,
) -> Result<MemberView, ServiceError> {
    let (workspace, standing) = resolve_manager(s, who, workspace_id).await?;
    let storage = &s.deps().storage;
    let role = WorkspaceRole::parse(&request.role)
        .ok_or_else(|| ServiceError::invalid("role must be owner or member"))?;
    let member = storage
        .get_member(&workspace.id, handle)
        .await
        .map_err(internal)?
        .filter(|m| m.kind == MemberKind::Person)
        .ok_or_else(|| ServiceError::NotFound("no such person in this Workspace".into()))?;
    if member.role == WorkspaceRole::Owner && role != WorkspaceRole::Owner && !standing.can_orphan() {
        let owners = storage
            .list_members(&workspace.id)
            .await
            .map_err(internal)?
            .iter()
            .filter(|m| m.kind == MemberKind::Person && m.role == WorkspaceRole::Owner)
            .count();
        if owners <= 1 {
            return Err(conflict("last_owner", "A Workspace needs at least one owner"));
        }
    }
    storage.set_member_role(&workspace.id, handle, role).await.map_err(internal)?;
    s.deps()
        .transport
        .publish_workspace_event(WorkspaceEvent::MemberChanged { workspace_id: workspace.id.clone(), removed: None });
    member_view(s, &WorkspaceMember { role, ..member }).await
}

pub async fn rename_topic(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
    topic_id: &str,
    request: RenameRequest,
) -> Result<TopicSummary, ServiceError> {
    let (workspace, _) = resolve_manager(s, who, workspace_id).await?;
    let topic = topic_of(s, &workspace, topic_id).await?;
    let name = names::normalize(&request.name);
    names::validate(&name).map_err(|e| name_error("topic", e))?;
    s.deps()
        .storage
        .rename_topic(&workspace.id, &topic.id, &name)
        .await
        .map_err(|e| name_conflict(e, "topic"))?;
    Ok(TopicSummary { id: topic.id, name, unread: 0 })
}

pub async fn delete_topic(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
    topic_id: &str,
) -> Result<(), ServiceError> {
    let (workspace, _) = resolve_manager(s, who, workspace_id).await?;
    let topic = topic_of(s, &workspace, topic_id).await?;
    let storage = &s.deps().storage;
    let topics = storage.list_topics(&workspace.id).await.map_err(internal)?;
    if topics.len() <= 1 {
        return Err(conflict("last_topic", "A Workspace needs at least one topic"));
    }
    let hub = Hub::from(s.deps());
    for member in storage.list_members(&workspace.id).await.map_err(internal)? {
        if member.kind == MemberKind::Agent {
            let session = workspaces::session_of(&member.handle, &workspace.id, &topic.id);
            workspaces::abort(&hub, &session).await;
            forget_session(s, &session).await;
        }
    }
    let _ = storage.delete_workspace_notifications(&workspace.id, Some(&topic.id)).await;
    storage.delete_topic(&workspace.id, &topic.id).await.map_err(internal)?;
    Ok(())
}

// --- searching for who to add -------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AgentCandidate {
    pub id: String,
    pub name: String,
    pub member: bool,
    pub addable: bool,
    /// `cannot_edit` for an agent the caller may see but not add.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PersonCandidate {
    pub username: String,
    pub name: String,
    pub member: bool,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CandidateGroup<T> {
    pub items: Vec<T>,
    /// How many more matched than were returned.
    pub more: usize,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Candidates {
    pub agents: CandidateGroup<AgentCandidate>,
    pub people: CandidateGroup<PersonCandidate>,
}

pub const DEFAULT_CANDIDATES: usize = 5;
pub const MAX_CANDIDATES: usize = 20;

/// 0 when the text starts the haystack, 1 when it is inside, none when absent.
fn match_rank(haystacks: &[&str], q: &str) -> Option<u8> {
    haystacks
        .iter()
        .filter_map(|h| {
            let h = h.to_lowercase();
            if h.starts_with(q) { Some(0) } else if h.contains(q) { Some(1) } else { None }
        })
        .min()
}

fn take<T>(mut ranked: Vec<(u8, T)>, limit: usize) -> CandidateGroup<T> {
    ranked.sort_by_key(|(rank, _)| *rank);
    let more = ranked.len().saturating_sub(limit);
    CandidateGroup { items: ranked.into_iter().take(limit).map(|(_, v)| v).collect(), more }
}

pub async fn candidates(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
    q: &str,
    limit: Option<usize>,
) -> Result<Candidates, ServiceError> {
    let (workspace, _) = resolve_manager(s, who, workspace_id).await?;
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return Err(ServiceError::invalid("q is required"));
    }
    let limit = limit.unwrap_or(DEFAULT_CANDIDATES).clamp(1, MAX_CANDIDATES);
    let storage = &s.deps().storage;
    let members = storage.list_members(&workspace.id).await.map_err(internal)?;
    let is_member = |handle: &str| members.iter().any(|m| m.handle == handle);

    // Order: addable, then not addable, then members; starts-with before contains within each.
    let mut agents = Vec::new();
    for (id, config) in storage.list_agents().await.map_err(internal)? {
        if !crate::access::can_view_agent(who, &config) {
            continue;
        }
        let Some(rank) = match_rank(&[&id, &config.name], &q) else { continue };
        let member = is_member(&id);
        let addable = !member && can_edit_agent(who, &config);
        let reason = (!member && !addable).then(|| "cannot_edit".to_string());
        let tier = if addable { 0 } else if !member { 1 } else { 2 };
        agents.push((tier * 2 + rank, AgentCandidate { id, name: config.name, member, addable, reason }));
    }
    let mut people = Vec::new();
    for user in storage.list_users().await.map_err(internal)? {
        let Some(rank) = match_rank(&[&user.username], &q) else { continue };
        let member = is_member(&user.username);
        people.push((u8::from(member) * 2 + rank, PersonCandidate { name: user.username.clone(), username: user.username, member }));
    }
    Ok(Candidates { agents: take(agents, limit), people: take(people, limit) })
}

// --- the conversation --------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AuthorView {
    /// `person` or `agent`.
    pub kind: String,
    pub id: String,
    pub name: String,
    /// An agent that was deleted since reads `false`; its messages stay.
    pub available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct MentionView {
    pub kind: String,
    pub handle: String,
    /// `pending`, `woken`, `unavailable`, `dropped` or `resolved`. The client draws a note only
    /// for `unavailable`.
    pub state: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct EntryView {
    pub seq: i64,
    /// `person`, `agent` or `note`.
    pub kind: String,
    pub author: AuthorView,
    pub body: String,
    /// Unix milliseconds.
    pub at: i64,
    pub mentions: Vec<MentionView>,
    /// Where it was sent from when not from inside: `{kind, label}`.
    pub origin: Option<serde_json::Value>,
    pub is_final: bool,
    pub activated_by_seq: Option<i64>,
    pub history_uid: Option<String>,
    pub files: Vec<crate::schema::EntryFile>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct StepView {
    pub agent_id: String,
    /// A history entry in the shape `GET …/history` returns.
    pub entry: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Timeline {
    /// Oldest first.
    pub entries: Vec<EntryView>,
    /// What the member agents did in the same span, to merge with `entries` by time.
    pub steps: Vec<StepView>,
    /// Pass as `before` for the page above; none when there is nothing older.
    pub next_before: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PostRequest {
    pub text: String,
    /// Ids of files already uploaded to this topic.
    #[serde(default)]
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PostResponse {
    /// The message posted. None when the text was a command.
    pub entry: Option<EntryView>,
    /// What a command did, shown as a centred line.
    pub note: Option<EntryView>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct StopResponse {
    pub stopped: usize,
    pub dropped: usize,
}

/// The default page and the most a client may ask for.
pub const DEFAULT_PAGE: usize = 50;
pub const MAX_PAGE: usize = 200;

fn post_error(e: PostError) -> ServiceError {
    match e {
        PostError::Empty => ServiceError::invalid("a message needs some text"),
        PostError::TooLong => refusal(
            422,
            "too_long",
            format!("A message can be at most {} characters", workspaces::MAX_MESSAGE_CHARS),
        ),
        PostError::Refused(reason) => refusal(422, "not_in_workspace", reason),
        PostError::Internal(m) => ServiceError::Internal(m),
    }
}

async fn topic_of(s: &Services, workspace: &Workspace, topic_id: &str) -> Result<WorkspaceTopic, ServiceError> {
    s.deps()
        .storage
        .get_topic(&workspace.id, topic_id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServiceError::NotFound("topic not found".into()))
}

/// Entries as a client reads them: author names, whether each agent still exists, and mentions.
pub async fn views(s: &Services, entries: &[WorkspaceEntry]) -> Result<Vec<EntryView>, ServiceError> {
    let storage = &s.deps().storage;
    let seqs: Vec<i64> = entries.iter().map(|e| e.seq).collect();
    let mentions: Vec<EntryMention> = storage.mentions_of(&seqs).await.map_err(internal)?;
    let mut alive: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let available = match entry.author_kind {
            MemberKind::Person => true,
            MemberKind::Agent => match alive.get(&entry.author) {
                Some(known) => *known,
                None => {
                    let known = storage.get_agent(&entry.author).await.map_err(internal)?.is_some();
                    alive.insert(entry.author.clone(), known);
                    known
                }
            },
        };
        out.push(EntryView {
            seq: entry.seq,
            kind: entry.kind.as_str().into(),
            author: AuthorView {
                kind: entry.author_kind.as_str().into(),
                id: entry.author.clone(),
                name: entry.author_name.clone(),
                available,
            },
            body: entry.body.clone(),
            at: entry.at,
            mentions: mentions
                .iter()
                .filter(|m| m.entry_seq == entry.seq)
                .map(|m| MentionView {
                    kind: m.target_kind.as_str().into(),
                    handle: m.target.clone(),
                    state: m.state.as_str().into(),
                })
                .collect(),
            origin: entry.origin.clone(),
            is_final: entry.is_final,
            activated_by_seq: entry.activated_by_seq,
            history_uid: entry.history_uid.clone(),
            files: entry.files.clone(),
        });
    }
    Ok(out)
}

pub async fn view_one(s: &Services, entry: &WorkspaceEntry) -> Result<EntryView, ServiceError> {
    Ok(views(s, std::slice::from_ref(entry)).await?.remove(0))
}

/// A page of a topic, with what the agents did in the same span.
pub async fn timeline(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
    topic_id: &str,
    before: Option<i64>,
    limit: Option<usize>,
) -> Result<Timeline, ServiceError> {
    let (workspace, _) = resolve(s, who, workspace_id).await?;
    let topic = topic_of(s, &workspace, topic_id).await?;
    let storage = &s.deps().storage;
    let limit = limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE);
    let entries = storage.list_entries(&topic.id, before, limit).await.map_err(internal)?;
    let next_before = (entries.len() == limit).then(|| entries[0].seq);
    let steps = match (entries.first(), entries.last()) {
        (Some(first), Some(last)) => {
            // The newest page reaches to now, so a turn still running shows its trail.
            let to = if before.is_none() { i64::MAX } else { last.at };
            let from = if next_before.is_some() { first.at } else { 0 };
            storage
                .steps_between(&workspace.id, &topic.id, from, to)
                .await
                .map_err(internal)?
                .into_iter()
                .map(|h| StepView {
                    agent_id: h.vizier_session.0.clone(),
                    entry: serde_json::to_value(&h).unwrap_or(serde_json::Value::Null),
                })
                .collect()
        }
        _ => vec![],
    };
    Ok(Timeline { entries: views(s, &entries).await?, steps, next_before })
}

fn member_agents(members: &[WorkspaceMember]) -> Vec<String> {
    members.iter().filter(|m| m.kind == MemberKind::Agent).map(|m| m.handle.clone()).collect()
}

async fn agent_name(s: &Services, id: &str) -> String {
    match s.deps().storage.get_agent(id).await {
        Ok(Some(agent)) => agent.name,
        _ => id.to_string(),
    }
}

async fn note(
    s: &Services,
    workspace: &Workspace,
    topic: &WorkspaceTopic,
    text: String,
) -> Result<EntryView, ServiceError> {
    let entry = workspaces::write_note(&Hub::from(s.deps()), &workspace.id, &topic.id, text)
        .await
        .map_err(post_error)?;
    view_one(s, &entry).await
}

/// The most files one message can carry.
pub const MAX_FILES: usize = 8;

/// Checks that each id is an upload that exists (from `POST /files/upload`) and names it.
async fn attach_files(s: &Services, ids: &[String]) -> Result<Vec<crate::schema::EntryFile>, ServiceError> {
    if ids.len() > MAX_FILES {
        return Err(refusal(422, "too_many_files", format!("A message can carry at most {MAX_FILES} files")));
    }
    let mut files = Vec::new();
    for id in ids {
        if files.iter().any(|f: &crate::schema::EntryFile| f.file_id == *id) {
            continue;
        }
        let blob = s
            .deps()
            .file_manager
            .locate(id)
            .await
            .map_err(internal)?
            .ok_or_else(|| refusal(422, "unknown_file", "One of the files was not uploaded"))?;
        let name = blob.meta.key.file_name().to_string();
        let mime_type = mime_guess::from_path(&name).first_or_octet_stream().to_string();
        files.push(crate::schema::EntryFile { file_id: id.clone(), name, mime_type, size: blob.meta.size });
    }
    Ok(files)
}

/// Posts a message, or runs the command it is (research R19). A command posts no message: it
/// answers a note.
pub async fn post(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
    topic_id: &str,
    request: PostRequest,
) -> Result<PostResponse, ServiceError> {
    let (workspace, standing) = resolve(s, who, workspace_id).await?;
    let topic = topic_of(s, &workspace, topic_id).await?;
    let hub = Hub::from(s.deps());
    let members = s.deps().storage.list_members(&workspace.id).await.map_err(internal)?;
    let agents = member_agents(&members);
    let agent_refs: Vec<&str> = agents.iter().map(String::as_str).collect();

    let parsed = commands::parse(&request.text, &agent_refs);
    let by = who.username.as_str();
    match &parsed {
        Parsed::StopAll => {
            workspaces::stop_all(&hub, &workspace.id).await.map_err(internal)?;
            let note = note(s, &workspace, &topic, format!("{by} stopped all agents")).await?;
            return Ok(PostResponse { entry: None, note: Some(note) });
        }
        Parsed::Abort { agents: targets } => {
            for target in targets {
                workspaces::stop_agent(&hub, &workspace.id, &topic.id, target).await.map_err(internal)?;
            }
            let names = join_names(s, targets).await;
            let note = note(s, &workspace, &topic, format!("{by} stopped {names}")).await?;
            return Ok(PostResponse { entry: None, note: Some(note) });
        }
        Parsed::Checkpoint { agents: targets } | Parsed::Lobotomy { agents: targets } => {
            let lobotomy = matches!(parsed, Parsed::Lobotomy { .. });
            if lobotomy {
                for target in targets {
                    // It wipes an agent's context, so it takes the right to edit the agent or to
                    // own the Workspace.
                    let may = standing.can_manage()
                        || match s.deps().storage.get_agent(target).await.map_err(internal)? {
                            Some(agent) => can_edit_agent(who, &agent),
                            None => false,
                        };
                    if !may {
                        return Err(refusal(
                            403,
                            "cannot_lobotomy",
                            "Only an owner of this Workspace, or someone who can edit the agent, can do that",
                        ));
                    }
                }
            }
            let word = if lobotomy { "lobotomy" } else { "checkpoint" };
            for target in targets {
                workspaces::send_command(&hub, &workspace.id, &topic.id, target, word).await;
            }
            let names = join_names(s, targets).await;
            let text = if lobotomy {
                format!("{by} reset {names}")
            } else {
                format!("{by} started a checkpoint for {names}")
            };
            let note = note(s, &workspace, &topic, text).await?;
            return Ok(PostResponse { entry: None, note: Some(note) });
        }
        _ => {}
    }

    let files = attach_files(s, &request.files).await?;
    let entry = workspaces::write_entry(
        &hub,
        NewEntry {
            workspace_id: workspace.id.clone(),
            topic_id: topic.id.clone(),
            kind: EntryKind::Person,
            author_kind: MemberKind::Person,
            author: who.username.clone(),
            author_name: who.username.clone(),
            body: request.text,
            files: files.clone(),
            history_uid: None,
            source_session: None,
            origin: None,
            is_final: false,
            shown: true,
            activated_by_seq: None,
            at: crate::time::now().timestamp_millis(),
        },
    )
    .await
    .map_err(post_error)?;
    // Every member agent has the files in its own record of the topic, so any of them can open
    // them with its file tools, whether it is woken now or reads this later.
    if !files.is_empty() {
        use crate::storage::session_file::SessionFileStorage;
        for member in members.iter().filter(|m| m.kind == MemberKind::Agent) {
            let session = workspaces::session_of(&member.handle, &workspace.id, &topic.id);
            for file in &files {
                let _ = s
                    .deps()
                    .storage
                    .save_session_file(&session, &file.name, &file.mime_type, file.size, &file.file_id)
                    .await;
            }
        }
    }
    Ok(PostResponse { entry: Some(view_one(s, &entry).await?), note: None })
}

async fn join_names(s: &Services, ids: &[String]) -> String {
    let mut names = Vec::new();
    for id in ids {
        names.push(agent_name(s, id).await);
    }
    names.join(", ")
}

/// Stop all (research R7).
pub async fn stop_all(s: &Services, who: &Principal, workspace_id: &str) -> Result<StopResponse, ServiceError> {
    let (workspace, _) = resolve(s, who, workspace_id).await?;
    let stopped = workspaces::stop_all(&Hub::from(s.deps()), &workspace.id).await.map_err(internal)?;
    // The note goes on the topic that has the newest activity; with none, the first.
    if let Some(topic) = s.deps().storage.list_topics(&workspace.id).await.map_err(internal)?.first() {
        let _ = workspaces::write_note(
            &Hub::from(s.deps()),
            &workspace.id,
            &topic.id,
            format!("{} stopped all agents", who.username),
        )
        .await;
    }
    Ok(StopResponse { stopped: stopped.stopped, dropped: stopped.dropped })
}

/// Stops one agent's turn in one topic.
pub async fn stop_agent(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
    topic_id: &str,
    agent_id: &str,
) -> Result<StopResponse, ServiceError> {
    let (workspace, _) = resolve(s, who, workspace_id).await?;
    let topic = topic_of(s, &workspace, topic_id).await?;
    let member = s.deps().storage.get_member(&workspace.id, agent_id).await.map_err(internal)?;
    if !member.is_some_and(|m| m.kind == MemberKind::Agent) {
        return Err(ServiceError::NotFound("no such agent in this Workspace".into()));
    }
    let stopped = workspaces::stop_agent(&Hub::from(s.deps()), &workspace.id, &topic.id, agent_id)
        .await
        .map_err(internal)?;
    let name = agent_name(s, agent_id).await;
    let _ = workspaces::write_note(
        &Hub::from(s.deps()),
        &workspace.id,
        &topic.id,
        format!("{} stopped {name}", who.username),
    )
    .await;
    Ok(StopResponse { stopped: stopped.stopped, dropped: stopped.dropped })
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ReactRequest {
    /// The history uid of an agent's reply in this topic.
    pub message_uid: String,
    pub emoji: String,
    /// `added` or `removed`.
    pub action: String,
}

/// A person's reaction on an agent's reply, as in a direct chat. It reaches the agent as context
/// and never starts a turn. The new set goes to every socket on the topic.
pub async fn react(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
    topic_id: &str,
    request: ReactRequest,
) -> Result<(), ServiceError> {
    use crate::channels::reactions::{self, ApplyOutcome, ReactionChange, ReactionKind, ReactionTarget};
    use crate::storage::reaction::{ReactionStorage, Reactor};
    let (workspace, _) = resolve(s, who, workspace_id).await?;
    let topic = topic_of(s, &workspace, topic_id).await?;
    let storage = &s.deps().storage;
    // Scoped to this topic, so a member cannot reach another conversation by guessing a uid.
    let in_topic = storage
        .get_history_entry(&request.message_uid)
        .await
        .map_err(internal)?
        .is_some_and(|entry| {
            let session = &entry.vizier_session;
            matches!(&session.1, crate::schema::VizierChannelId::Workspace(id) if *id == workspace.id)
                && session.2.as_deref() == Some(topic.id.as_str())
        });
    if !in_topic {
        return Err(ServiceError::NotFound("message not found in this topic".into()));
    }
    let kind = match request.action.as_str() {
        "added" => ReactionKind::Add(request.emoji),
        "removed" => ReactionKind::Remove(request.emoji),
        _ => return Err(ServiceError::invalid("action must be added or removed")),
    };
    let change = ReactionChange {
        target: ReactionTarget::History(request.message_uid),
        reactor: Reactor { id: who.username.clone(), name: None },
        kind,
    };
    match reactions::apply(storage, &s.deps().transport, change).await.map_err(internal)? {
        ApplyOutcome::Applied => Ok(()),
        ApplyOutcome::Ignored(reason) => Err(ServiceError::invalid(reason)),
    }
}

// --- the `/` menu --------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CommandView {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SkillView {
    pub name: String,
    pub description: String,
    /// The member agents that have it.
    pub agents: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CommandsView {
    pub commands: Vec<CommandView>,
    pub skills: Vec<SkillView>,
}

/// Names and descriptions only, never a skill's content: adding an agent already shared its use
/// with every member (FR-005), and this tells them what they can ask of it.
pub async fn commands_menu(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
) -> Result<CommandsView, ServiceError> {
    let (workspace, _) = resolve(s, who, workspace_id).await?;
    let members = s.deps().storage.list_members(&workspace.id).await.map_err(internal)?;
    let mut skills: Vec<SkillView> = Vec::new();
    for agent in member_agents(&members) {
        let scopes = crate::skill::scopes::SkillScopes::workspace(&s.deps().config.workspace, &agent);
        for scoped in scopes.available().into_iter().filter(|k| k.skill.user_invocable) {
            match skills.iter_mut().find(|k| k.name == scoped.skill.name) {
                Some(known) => known.agents.push(agent.clone()),
                None => skills.push(SkillView {
                    name: scoped.skill.name.clone(),
                    description: scoped.skill.description.clone(),
                    agents: vec![agent.clone()],
                }),
            }
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    let command = |name: &str, description: &str| CommandView {
        name: name.into(),
        description: description.into(),
    };
    Ok(CommandsView {
        commands: vec![
            command("skills", "List an agent's skills"),
            command("abort", "Stop an agent's turn, or all of them"),
            command("checkpoint", "Save an agent's context with a handover"),
            command("lobotomy", "Reset an agent's context with no handover"),
        ],
        skills,
    })
}

// --- the live stream -----------------------------------------------------------------------------

/// What one event becomes for one connection.
pub enum Delivery {
    /// Frames to send, possibly none.
    Frames(Vec<serde_json::Value>),
    /// The person is out, or the Workspace is gone: send `closed` and end the connection.
    Close(&'static str),
}

fn frame(name: &str, body: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ name: body })
}

/// The first frame of a connection: where the timeline ends and who is working. A client reads
/// what it missed by `seq` (`specs/053-workspace-channel/contracts/socket-frames.md`).
pub async fn hello(
    s: &Services,
    who: &Principal,
    workspace_id: &str,
    topic_id: &str,
) -> Result<serde_json::Value, ServiceError> {
    let (workspace, _) = resolve(s, who, workspace_id).await?;
    let topic = topic_of(s, &workspace, topic_id).await?;
    let latest_seq = s.deps().storage.head_seq(&workspace.id).await.map_err(internal)?;
    let working: Vec<serde_json::Value> = busy_agents(s, &workspace.id, &topic.id)
        .await
        .into_iter()
        .map(|agent| serde_json::json!({ "agent": agent, "phase": "thinking" }))
        .collect();
    Ok(frame("hello", serde_json::json!({ "latest_seq": latest_seq, "working": working })))
}

/// Agents with a turn running or queued in one topic.
pub async fn busy_agents(s: &Services, workspace_id: &str, topic_id: &str) -> Vec<String> {
    use crate::storage::turn::TurnStorage;
    s.deps()
        .storage
        .sessions_with_pending_turns()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|session| {
            matches!(&session.1, crate::schema::VizierChannelId::Workspace(id) if id == workspace_id)
                && session.2.as_deref() == Some(topic_id)
        })
        .map(|session| session.0)
        .collect()
}

/// Membership is re-checked for every delivery, so a removal takes effect on an open socket at
/// the next event (FR-004, SC-006).
async fn still_a_member(
    s: &Services,
    user_id: &str,
    method: crate::auth::AuthMethod,
    workspace_id: &str,
) -> Option<Principal> {
    let who = s.resolve(user_id, method).await.ok()?;
    resolve(s, &who, workspace_id).await.ok()?;
    Some(who)
}

pub async fn deliver_workspace(
    s: &Services,
    user_id: &str,
    method: crate::auth::AuthMethod,
    workspace_id: &str,
    topic_id: &str,
    event: &WorkspaceEvent,
) -> Delivery {
    if event.workspace_id() != workspace_id {
        return Delivery::Frames(vec![]);
    }
    if matches!(event, WorkspaceEvent::Deleted { .. }) {
        return Delivery::Close("deleted");
    }
    if still_a_member(s, user_id, method, workspace_id).await.is_none() {
        return Delivery::Close("removed");
    }
    match event {
        WorkspaceEvent::Entry { entry, .. } if entry.topic_id == topic_id => {
            match view_one(s, entry).await {
                Ok(view) => Delivery::Frames(vec![frame(
                    "workspace_entry",
                    serde_json::to_value(view).unwrap_or_default(),
                )]),
                Err(_) => Delivery::Frames(vec![]),
            }
        }
        WorkspaceEvent::Mentions { topic_id: t, seq, mentions, .. } if t == topic_id => {
            let views: Vec<MentionView> = mentions
                .iter()
                .map(|m| MentionView {
                    kind: m.target_kind.as_str().into(),
                    handle: m.target.clone(),
                    state: m.state.as_str().into(),
                })
                .collect();
            Delivery::Frames(vec![frame(
                "workspace_mentions",
                serde_json::json!({ "seq": seq, "mentions": views }),
            )])
        }
        WorkspaceEvent::MemberChanged { .. } => {
            Delivery::Frames(vec![frame("member_changed", serde_json::json!({}))])
        }
        _ => Delivery::Frames(vec![]),
    }
}

pub async fn deliver_session(
    s: &Services,
    user_id: &str,
    method: crate::auth::AuthMethod,
    workspace_id: &str,
    topic_id: &str,
    event: &crate::transport::SessionEvent,
) -> Delivery {
    use crate::schema::{VizierChannelId, VizierResponseContent as Content};
    use crate::transport::SessionFrame;
    let session = &event.session;
    if !matches!(&session.1, VizierChannelId::Workspace(id) if id == workspace_id)
        || session.2.as_deref() != Some(topic_id)
    {
        return Delivery::Frames(vec![]);
    }
    if still_a_member(s, user_id, method, workspace_id).await.is_none() {
        return Delivery::Close("removed");
    }
    let agent = session.0.as_str();
    let frames = match &event.frame {
        // A step of a running turn, tagged with the agent that took it.
        SessionFrame::HistoryEntry(entry) => vec![frame(
            "history_entry",
            serde_json::json!({ "agent_id": agent, "entry": entry }),
        )],
        // A turn's text arrives as a `workspace_entry` once saved, so a reply frame says only
        // whether the agent is working.
        SessionFrame::Response(response) => {
            let phase = match &response.content {
                Content::ThinkingStart
                | Content::Thinking(_)
                | Content::ToolChoice { .. }
                | Content::ToolResponse { .. } => "thinking",
                Content::Message { .. }
                | Content::AudioReply(..)
                | Content::Error { .. }
                | Content::Empty
                | Content::Abort => "idle",
                Content::Checkpoint { .. } => return Delivery::Frames(vec![]),
            };
            vec![frame("agent_state", serde_json::json!({ "agent": agent, "phase": phase }))]
        }
        SessionFrame::Reactions { history_uid, reactions } => vec![frame(
            "reactions",
            serde_json::json!({ "message_uid": history_uid, "reactions": reactions }),
        )],
        _ => vec![],
    };
    Delivery::Frames(frames)
}
