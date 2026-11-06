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
    schema::{MemberKind, Workspace, WorkspaceMember, WorkspaceRole, WorkspaceTopic},
    storage::{agent::AgentStorage, user::UserStorage, workspace::WorkspaceStorage},
    workspaces::{
        access::{self, Standing},
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
    workspace: &Workspace,
    standing: Standing,
) -> Result<WorkspaceSummary, ServiceError> {
    let topics = s
        .deps()
        .storage
        .list_topics(&workspace.id)
        .await
        .map_err(internal)?
        .into_iter()
        .map(|t| TopicSummary { id: t.id, name: t.name, unread: 0 })
        .collect();
    Ok(WorkspaceSummary {
        id: workspace.id.clone(),
        name: workspace.name.clone(),
        role: role_word(standing).into(),
        unread: 0,
        working: false,
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
        rows.push(summary(s, &workspace, standing).await?);
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
    Ok(WorkspaceDetail { workspace: summary(s, &workspace, standing).await?, members })
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
        .map_err(internal)
}
