//! Projects over the service layer (`specs/040-projects-mode/contracts/http-api.md`): list, open,
//! detail, rename, the shell switch, soft removal and a person's sessions. Authorization and
//! folder validation live here; handlers only translate.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{ServiceError, Services};
use crate::{
    access::Caller,
    auth::Principal,
    projects::{access, channel, instructions},
    schema::{
        AgentConfig, AgentId, NEW_SESSION_TITLE, PROJECT_ID_LEN, Project, ProjectSessionRow,
        TopicId, VizierChannelId, VizierSession, VizierSessionDetail,
    },
    storage::{
        agent::AgentStorage,
        project::ProjectStorage,
        session::SessionStorage,
        turn::TurnStorage,
    },
};

/// The longest project name the UI and the service accept.
pub const MAX_NAME_CHARS: usize = 120;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProjectSummary {
    pub id: String,
    pub agent_id: AgentId,
    pub name: String,
    /// The canonical folder, with Windows' `\\?\` prefix stripped for display.
    pub folder: String,
    pub available: bool,
    pub shell_enabled: bool,
    pub owner_id: String,
    pub is_owner: bool,
    /// Sessions in the project across every owner (the sidebar's project row).
    pub session_count: usize,
    /// Unix milliseconds.
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProjectDetail {
    #[serde(flatten)]
    pub project: ProjectSummary,
    pub instructions_present: bool,
    pub instructions_truncated: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OpenProjectRequest {
    /// The folder to mount. Its final component becomes the project name.
    pub folder: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateProjectRequest {
    /// Trimmed, 1–120 characters.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub shell_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProjectSession {
    pub topic: String,
    pub title: String,
    pub is_thinking: bool,
    /// Unix milliseconds; `null` for a brand-new empty session.
    pub last_activity: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateProjectSessionResponse {
    pub topic: String,
    pub title: String,
    pub channel_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProjectRemoved {
    pub removed: bool,
}

/// `open`'s outcome: the summary and whether a row was inserted (the API's `201`).
#[derive(Debug, Clone)]
pub struct OpenedProject {
    pub project: ProjectSummary,
    pub created: bool,
}

fn internal(err: impl std::fmt::Display) -> ServiceError {
    ServiceError::Internal(err.to_string())
}

fn invalid_code(code: &str, message: impl Into<String>) -> ServiceError {
    ServiceError::Invalid {
        status: 422,
        message: message.into(),
        code: Some(code.into()),
    }
}

/// The agent, when `who` may use its projects: `404` when it does not exist, `403` when the
/// person may not view it or lacks a project permission.
async fn agent_for_use(
    s: &Services,
    who: &Principal,
    agent_id: &str,
) -> Result<AgentConfig, ServiceError> {
    let agent = match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(agent)) => agent,
        Ok(None) => return Err(ServiceError::NotFound(format!("agent {agent_id} not found"))),
        Err(err) => return Err(internal(err)),
    };
    if !access::can_use(Caller::from(who), &agent) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    Ok(agent)
}

/// The agent and a non-removed project of it, or `404`/`403`.
async fn project_for_read(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
) -> Result<Project, ServiceError> {
    agent_for_use(s, who, agent_id).await?;
    match s.deps().storage.get_project(agent_id, project_id).await {
        Ok(Some(project)) if project.removed_at.is_none() => Ok(project),
        Ok(_) => Err(ServiceError::NotFound("project not found".into())),
        Err(err) => Err(internal(err)),
    }
}

/// [`project_for_read`], plus the right to rename, remove or toggle it.
async fn project_for_manage(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
) -> Result<Project, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    if !access::can_manage(Caller::from(who), &project) {
        return Err(ServiceError::Forbidden(
            "only the project owner or a whole-workspace project manager may change this project"
                .into(),
        ));
    }
    Ok(project)
}

/// Canonicalize a chosen folder and refuse it when it is missing, not a directory, or the
/// application's own workspace (or inside it).
fn validate_folder(s: &Services, folder: &str) -> Result<PathBuf, ServiceError> {
    let folder = folder.trim();
    if folder.is_empty() {
        return Err(invalid_code("invalid_folder", "choose an existing folder"));
    }
    let canonical = channel::canonical_folder(Path::new(folder))
        .ok_or_else(|| invalid_code("invalid_folder", "the folder does not exist or is not a directory"))?;
    let workspace = PathBuf::from(&s.deps().config.workspace);
    let workspace = std::fs::canonicalize(&workspace).unwrap_or(workspace);
    if channel::inside(&canonical, &workspace) {
        return Err(invalid_code(
            "workspace_folder",
            "the application's own workspace cannot be opened as a project",
        ));
    }
    Ok(canonical)
}

/// A trimmed name of 1–120 characters.
fn validate_name(name: &str) -> Result<String, ServiceError> {
    let name = name.trim();
    let len = name.chars().count();
    if !(1..=MAX_NAME_CHARS).contains(&len) {
        return Err(invalid_code(
            "invalid_name",
            format!("the project name must be 1–{MAX_NAME_CHARS} characters"),
        ));
    }
    Ok(name.to_string())
}

/// The name a folder gets when it is opened: its final component.
fn folder_name(folder: &Path) -> Result<String, ServiceError> {
    let name = folder
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .trim();
    match name.is_empty() {
        true => Err(invalid_code("invalid_name", "the folder has no usable name")),
        false => validate_name(name),
    }
}

/// One project as the API shows it, with its session count.
async fn summary(s: &Services, who: &Principal, project: Project) -> Result<ProjectSummary, ServiceError> {
    let session_count = s
        .deps()
        .storage
        .project_session_count(&project.agent_id, &project.id)
        .await
        .map_err(internal)?;
    Ok(ProjectSummary {
        id: project.id.clone(),
        agent_id: project.agent_id.clone(),
        name: project.name.clone(),
        folder: project.display_folder(),
        available: project.available(),
        shell_enabled: project.shell_enabled,
        is_owner: project.owner_id == who.user_id,
        owner_id: project.owner_id.clone(),
        session_count,
        created_at: project.created_at,
    })
}

/// The agent's open projects, newest first.
pub async fn list(
    s: &Services,
    who: &Principal,
    agent_id: &str,
) -> Result<Vec<ProjectSummary>, ServiceError> {
    agent_for_use(s, who, agent_id).await?;
    let projects = s
        .deps()
        .storage
        .list_projects(agent_id, false)
        .await
        .map_err(internal)?;
    let mut out = Vec::with_capacity(projects.len());
    for project in projects {
        out.push(summary(s, who, project).await?);
    }
    Ok(out)
}

/// A project's detail. The instruction text is never returned, only whether it exists.
pub async fn get(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
) -> Result<ProjectDetail, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    let instructions = match project.available() {
        true => instructions::read(&project.folder),
        false => None,
    };
    Ok(ProjectDetail {
        project: summary(s, who, project).await?,
        instructions_present: instructions.is_some(),
        instructions_truncated: instructions.is_some_and(|read| read.truncated),
    })
}

/// Open a folder as a project. Desktop only; the same folder for the same agent reuses (or
/// restores) the existing row.
pub async fn open(
    s: &Services,
    who: &Principal,
    method: crate::auth::AuthMethod,
    agent_id: &str,
    request: OpenProjectRequest,
) -> Result<OpenedProject, ServiceError> {
    let agent = agent_for_use(s, who, agent_id).await?;
    if !access::can_create(Caller::from(who), &agent, method) {
        return Err(ServiceError::Invalid {
            status: 403,
            message: "creating a project requires the desktop app".into(),
            code: Some("desktop_only".into()),
        });
    }
    let folder = validate_folder(s, &request.folder)?;
    let name = folder_name(&folder)?;

    let now = chrono::Utc::now().timestamp_millis();
    let project = Project {
        id: nanoid::nanoid!(PROJECT_ID_LEN),
        agent_id: agent_id.to_string(),
        name,
        folder,
        owner_id: who.user_id.clone(),
        shell_enabled: true,
        created_at: now,
        removed_at: None,
    };
    let (project, created) = s.deps().storage.open_project(project).await.map_err(internal)?;
    Ok(OpenedProject {
        project: summary(s, who, project).await?,
        created,
    })
}

/// Rename and/or toggle the workspace shell.
pub async fn update(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    request: UpdateProjectRequest,
) -> Result<ProjectSummary, ServiceError> {
    project_for_manage(s, who, agent_id, project_id).await?;
    if request.name.is_none() && request.shell_enabled.is_none() {
        return Err(invalid_code("invalid_update", "provide a name or a shell switch"));
    }
    if let Some(name) = request.name {
        let name = validate_name(&name)?;
        s.deps()
            .storage
            .rename_project(project_id, &name)
            .await
            .map_err(internal)?;
    }
    if let Some(enabled) = request.shell_enabled {
        s.deps()
            .storage
            .set_project_shell(project_id, enabled)
            .await
            .map_err(internal)?;
    }
    let project = s
        .deps()
        .storage
        .get_project(agent_id, project_id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServiceError::NotFound("project not found".into()))?;
    summary(s, who, project).await
}

/// Rename a project.
pub async fn rename(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    name: &str,
) -> Result<ProjectSummary, ServiceError> {
    update(
        s,
        who,
        agent_id,
        project_id,
        UpdateProjectRequest {
            name: Some(name.to_string()),
            shell_enabled: None,
        },
    )
    .await
}

/// Turn the workspace shell on or off. Takes effect on the next turn; no agent restart.
pub async fn set_shell(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    enabled: bool,
) -> Result<ProjectSummary, ServiceError> {
    update(
        s,
        who,
        agent_id,
        project_id,
        UpdateProjectRequest {
            name: None,
            shell_enabled: Some(enabled),
        },
    )
    .await
}

/// Soft-remove a project, refusing while any turn on its channel is queued or running.
pub async fn remove(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
) -> Result<ProjectRemoved, ServiceError> {
    let project = project_for_manage(s, who, agent_id, project_id).await?;
    let channel_id = channel::channel_id(&project.id);
    let pending = s
        .deps()
        .storage
        .pending_turns(agent_id)
        .await
        .map_err(internal)?;
    if pending.iter().any(|turn| {
        matches!(&turn.session.1, VizierChannelId::HTTP(_, id) if *id == channel_id)
    }) {
        return Err(ServiceError::Conflict {
            message: "a turn is running or queued in this project; wait for it to finish".into(),
            code: Some("turn_active".into()),
            data: None,
        });
    }
    s.deps()
        .storage
        .remove_project(&project.id, chrono::Utc::now().timestamp_millis())
        .await
        .map_err(internal)?;
    Ok(ProjectRemoved { removed: true })
}

/// The caller's own sessions in a project, newest activity first.
pub async fn sessions(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
) -> Result<Vec<ProjectSession>, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    let slug = VizierChannelId::HTTP(
        who.username.clone(),
        channel::channel_id(&project.id),
    )
    .to_slug();
    let rows = s
        .deps()
        .storage
        .project_sessions(agent_id, &slug)
        .await
        .map_err(internal)?;
    Ok(rows.into_iter().map(session_item).collect())
}

fn session_item(row: ProjectSessionRow) -> ProjectSession {
    ProjectSession {
        topic: row.topic,
        title: row.title,
        is_thinking: row.is_thinking,
        last_activity: row.last_activity,
    }
}

/// Create an empty titled session for the caller, so it is listed before its first message.
pub async fn create_session(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
) -> Result<CreateProjectSessionResponse, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    let channel_id = channel::channel_id(&project.id);
    let topic: TopicId = nanoid::nanoid!(PROJECT_ID_LEN);
    let detail = VizierSessionDetail {
        agent_id: agent_id.to_string(),
        channel: VizierChannelId::HTTP(who.username.clone(), channel_id.clone()),
        topic: Some(topic.clone()),
        title: NEW_SESSION_TITLE.to_string(),
        is_thinking: false,
    };
    s.deps()
        .storage
        .save_session_detail(detail)
        .await
        .map_err(internal)?;
    Ok(CreateProjectSessionResponse {
        topic,
        title: NEW_SESSION_TITLE.to_string(),
        channel_id,
    })
}

/// The session a project topic addresses, for the turn-side authorization paths.
pub fn session_for(who: &Principal, agent_id: &str, project_id: &str, topic: &str) -> VizierSession {
    VizierSession(
        agent_id.to_string(),
        VizierChannelId::HTTP(who.username.clone(), channel::channel_id(project_id)),
        Some(topic.to_string()),
    )
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::{
        auth::AuthMethod,
        config::VizierConfig,
        schema::{VizierRequest, VizierRequestContent},
        storage::user::UserStorage,
    };

    async fn setup() -> (Services, Principal, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let deps = crate::dependencies::VizierDependencies::new(VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        let role = deps
            .storage
            .create_role(
                "projects",
                vec!["all_agents:view".into(), access::USE_ALL.into()],
                false,
            )
            .await
            .unwrap();
        deps.storage
            .create_user("alice", "hash", &role.role_id)
            .await
            .unwrap();
        deps.storage
            .create_agent("dp-a", &crate::tests_support::agent_config("dp-a"))
            .await
            .unwrap();
        let who = Principal {
            user_id: "u-alice".into(),
            username: "alice".into(),
            role: role.clone(),
            permissions: role.permissions.clone(),
            method: AuthMethod::Desktop,
        };
        // The principal's user id must be the stored one.
        let stored = deps.storage.get_user("alice").await.unwrap().unwrap();
        let who = Principal { user_id: stored.user_id, ..who };
        (Services::new(deps), who, dir)
    }

    fn folder() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn request(dir: &tempfile::TempDir) -> OpenProjectRequest {
        OpenProjectRequest {
            folder: dir.path().to_string_lossy().into_owned(),
        }
    }

    #[tokio::test]
    async fn opening_reuses_and_restores_the_same_project() {
        let (services, who, _dir) = setup().await;
        let folder = folder();
        let first = open(&services, &who, AuthMethod::Desktop, "dp-a", request(&folder))
            .await
            .unwrap();
        assert!(first.created);
        let again = open(&services, &who, AuthMethod::Desktop, "dp-a", request(&folder))
            .await
            .unwrap();
        assert!(!again.created);
        assert_eq!(first.project.id, again.project.id);

        let created = create_session(&services, &who, "dp-a", &first.project.id).await.unwrap();
        remove(&services, &who, "dp-a", &first.project.id).await.unwrap();
        assert!(list(&services, &who, "dp-a").await.unwrap().is_empty());

        let restored = open(&services, &who, AuthMethod::Desktop, "dp-a", request(&folder))
            .await
            .unwrap();
        assert!(!restored.created);
        assert_eq!(restored.project.id, first.project.id);
        let sessions = sessions(&services, &who, "dp-a", &first.project.id).await.unwrap();
        assert_eq!(sessions[0].topic, created.topic);
        assert_eq!(sessions[0].title, NEW_SESSION_TITLE);
    }

    #[tokio::test]
    async fn creating_needs_a_desktop_principal() {
        let (services, who, _dir) = setup().await;
        let folder = folder();
        let err = open(&services, &who, AuthMethod::Jwt, "dp-a", request(&folder))
            .await
            .unwrap_err();
        assert_eq!(err.status(), 403);
        assert_eq!(err.code(), Some("desktop_only"));
    }

    #[tokio::test]
    async fn a_missing_or_workspace_folder_is_refused() {
        let (services, who, dir) = setup().await;
        let err = open(
            &services,
            &who,
            AuthMethod::Desktop,
            "dp-a",
            OpenProjectRequest { folder: "/no/such/folder".into() },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code(), Some("invalid_folder"));

        // The application's own workspace, and a folder inside it.
        for folder in [dir.path().to_path_buf(), dir.path().join("nested")] {
            if folder != *dir.path() {
                std::fs::create_dir(&folder).unwrap();
            }
            let err = open(
                &services,
                &who,
                AuthMethod::Desktop,
                "dp-a",
                OpenProjectRequest { folder: folder.to_string_lossy().into_owned() },
            )
            .await
            .unwrap_err();
            assert_eq!(err.code(), Some("workspace_folder"), "{folder:?}");
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_symlink_into_the_workspace_is_refused() {
        let (services, who, dir) = setup().await;
        let link = dir.path().join("links");
        std::fs::create_dir(&link).unwrap();
        let target = dir.path().join("secret");
        std::fs::create_dir(&target).unwrap();
        let link = link.join("escape");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let err = open(
            &services,
            &who,
            AuthMethod::Desktop,
            "dp-a",
            OpenProjectRequest { folder: link.to_string_lossy().into_owned() },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code(), Some("workspace_folder"));
    }

    #[test]
    fn names_are_trimmed_and_bounded() {
        assert_eq!(validate_name("  alpha  ").unwrap(), "alpha");
        assert_eq!(validate_name("").unwrap_err().code(), Some("invalid_name"));
        assert_eq!(validate_name(&"x".repeat(121)).unwrap_err().code(), Some("invalid_name"));
        assert_eq!(validate_name(&"x".repeat(120)).unwrap().len(), 120);
    }

    #[tokio::test]
    async fn removal_is_refused_while_a_project_turn_is_pending() {
        let (services, who, _dir) = setup().await;
        let folder = folder();
        let opened = open(&services, &who, AuthMethod::Desktop, "dp-a", request(&folder))
            .await
            .unwrap();
        let created = create_session(&services, &who, "dp-a", &opened.project.id).await.unwrap();
        let session = session_for(&who, "dp-a", &opened.project.id, &created.topic);
        let request = VizierRequest {
            content: VizierRequestContent::Chat("hi".into()),
            ..Default::default()
        };
        services.deps().storage.accept_turn(&session, &request).await.unwrap();

        let err = remove(&services, &who, "dp-a", &opened.project.id).await.unwrap_err();
        assert_eq!(err.status(), 409);
        assert_eq!(err.code(), Some("turn_active"));
        assert_eq!(list(&services, &who, "dp-a").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_non_owner_without_the_all_permission_cannot_manage() {
        let (services, who, _dir) = setup().await;
        let folder = folder();
        let opened = open(&services, &who, AuthMethod::Desktop, "dp-a", request(&folder))
            .await
            .unwrap();

        let role = services
            .deps()
            .storage
            .create_role("owned", vec![access::USE_OWNED.into()], false)
            .await
            .unwrap();
        let bob = services.deps().storage.create_user("bob", "hash", &role.role_id).await.unwrap();
        // Bob owns the agent (so he may use its projects) but not the project Alice opened.
        let mut agent = services.deps().storage.get_agent("dp-a").await.unwrap().unwrap();
        agent.owner_id = Some(bob.user_id.clone());
        services.deps().storage.update_agent("dp-a", &agent).await.unwrap();
        let bob = Principal {
            user_id: bob.user_id,
            username: "bob".into(),
            role: role.clone(),
            permissions: role.permissions.clone(),
            method: AuthMethod::Desktop,
        };
        assert!(list(&services, &bob, "dp-a").await.is_ok());
        let err = rename(&services, &bob, "dp-a", &opened.project.id, "mine").await.unwrap_err();
        assert_eq!(err.status(), 403);
    }

    #[tokio::test]
    async fn sessions_are_per_person() {
        let (services, who, _dir) = setup().await;
        let folder = folder();
        let opened = open(&services, &who, AuthMethod::Desktop, "dp-a", request(&folder))
            .await
            .unwrap();
        create_session(&services, &who, "dp-a", &opened.project.id).await.unwrap();
        assert_eq!(sessions(&services, &who, "dp-a", &opened.project.id).await.unwrap().len(), 1);

        let role = services
            .deps()
            .storage
            .create_role("owned", vec!["all_agents:view".into(), access::USE_ALL.into()], false)
            .await
            .unwrap();
        let bob = services.deps().storage.create_user("bob", "hash", &role.role_id).await.unwrap();
        let bob = Principal {
            user_id: bob.user_id,
            username: "bob".into(),
            role: role.clone(),
            permissions: role.permissions.clone(),
            method: AuthMethod::Desktop,
        };
        // Bob sees the project row but none of Alice's sessions (FR-024).
        assert_eq!(list(&services, &bob, "dp-a").await.unwrap().len(), 1);
        assert!(sessions(&services, &bob, "dp-a", &opened.project.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn detail_reports_instructions_without_text() {
        let (services, who, _dir) = setup().await;
        let folder = folder();
        std::fs::write(folder.path().join("AGENTS.md"), "Be terse.").unwrap();
        let opened = open(&services, &who, AuthMethod::Desktop, "dp-a", request(&folder))
            .await
            .unwrap();
        let detail = get(&services, &who, "dp-a", &opened.project.id).await.unwrap();
        assert!(detail.instructions_present);
        let text = serde_json::to_string(&detail).unwrap();
        assert!(!text.contains("Be terse."));
        assert!(Utc::now().timestamp_millis() >= detail.project.created_at);
    }
}
