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
        AgentConfig, AgentId, NEW_SESSION_TITLE, PROJECT_ID_LEN, Project, ProjectMode,
        ProjectSessionRow, TopicId, VizierChannelId, VizierSession, VizierSessionDetail, Worktree,
        WorktreeRef,
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
    /// Worktree chats whose folders stay on disk when the project is removed (FR-033).
    pub worktree_count: usize,
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
    /// The chat's saved Plan/Build mode (FR-009).
    pub mode: ProjectMode,
    /// The worktree this chat works in, when it has one.
    pub worktree: Option<WorktreeSummary>,
}

/// A worktree as the sidebar and a create response show it.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct WorktreeSummary {
    pub id: String,
    pub name: String,
    pub branch: String,
    /// The folder is still there (FR-032).
    pub available: bool,
}

impl WorktreeSummary {
    fn of(worktree: &Worktree) -> Self {
        Self {
            id: worktree.id.clone(),
            name: worktree.name.clone(),
            branch: worktree.branch.clone(),
            available: worktree.path.is_dir(),
        }
    }
}

fn worktree_ref_summary(worktree: &WorktreeRef) -> WorktreeSummary {
    WorktreeSummary {
        id: worktree.id.clone(),
        name: worktree.name.clone(),
        branch: worktree.branch.clone(),
        available: true,
    }
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
    let worktree_count = s
        .deps()
        .storage
        .worktrees(&project.id)
        .await
        .map_err(internal)?
        .len();
    Ok(ProjectDetail {
        project: summary(s, who, project).await?,
        instructions_present: instructions.is_some(),
        instructions_truncated: instructions.is_some_and(|read| read.truncated),
        worktree_count,
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
        turn.session.1.web_channel_id().as_deref() == Some(channel_id.as_str())
            && turn.session.1.project_id().is_some()
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
    let slug = VizierChannelId::from_web(&who.username, &channel::channel_id(&project.id)).to_slug();
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
        mode: row.mode,
        worktree: row.worktree.as_ref().map(worktree_ref_summary),
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SetModeRequest {
    pub mode: ProjectMode,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SetModeResponse {
    pub mode: ProjectMode,
}

/// Switch one of the caller's chats between Plan and Build. Effective from the next turn; a
/// running turn keeps the tools it started with (FR-013).
pub async fn set_mode(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    topic: &str,
    request: SetModeRequest,
) -> Result<SetModeResponse, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    let channel = VizierChannelId::Project(who.username.clone(), project.id.clone()).to_slug();
    s.deps()
        .storage
        .set_project_chat_mode(agent_id, &channel, topic, request.mode)
        .await
        .map_err(internal)?;
    Ok(SetModeResponse { mode: request.mode })
}

/// The folder a chat works in: its worktree when bound to one, else the project root.
async fn chat_cwd(
    s: &Services,
    project: &Project,
    topic: Option<&str>,
) -> Result<PathBuf, ServiceError> {
    let worktree = match topic {
        Some(topic) => s
            .deps()
            .storage
            .worktree_by_topic(&project.id, topic)
            .await
            .map_err(internal)?,
        None => None,
    };
    Ok(match worktree {
        Some(worktree) => worktree.path,
        None => project.folder.clone(),
    })
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
        channel: VizierChannelId::from_web(&who.username, &channel_id),
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
        VizierChannelId::from_web(&who.username, &channel::channel_id(project_id)),
        Some(topic.to_string()),
    )
}


#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct FileSuggestion {
    pub path: String,
    /// `file` or `dir`.
    pub kind: String,
}

/// Path suggestions for the composer's `@` menu (FR-017), from the chat's own folder.
pub async fn file_suggestions(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    query: &str,
    topic: Option<&str>,
    limit: usize,
) -> Result<Vec<FileSuggestion>, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    let cwd = chat_cwd(s, &project, topic).await?;
    if !cwd.is_dir() {
        return Err(ServiceError::Conflict {
            message: "the project folder is missing".into(),
            code: Some("project_unavailable".into()),
            data: None,
        });
    }
    let query = query.to_string();
    let suggestions = tokio::task::spawn_blocking(move || {
        crate::projects::suggest::suggest(&cwd, &query, limit.clamp(1, 50))
    })
    .await
    .map_err(|err| ServiceError::Internal(err.to_string()))?;
    Ok(suggestions
        .into_iter()
        .map(|suggestion| FileSuggestion {
            path: suggestion.path,
            kind: if suggestion.is_dir { "dir" } else { "file" }.to_string(),
        })
        .collect())
}

/// One skill a project chat can run, as the composer's `/` menu lists it.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProjectSkillRow {
    pub name: String,
    pub description: String,
    /// `project`. Every row here is one, and saying so keeps the shape the same if a later route
    /// merges the workspace scopes in.
    pub scope: String,
    /// Whether a person may run it by name; a `false` one is prose for the agent, not a command.
    pub user_invocable: bool,
    /// The usage line the skill declares, e.g. `[audit|critique] [target]`.
    pub argument_hint: Option<String>,
    /// Its folder relative to the chat's folder, which is what `use_skill` reports.
    pub folder: String,
}

/// The skills a project chat carries in its own folder (`specs/041-project-coding-workspace`
/// FR-035), read from the folder the chat works in — its worktree's own checkout when it is bound
/// to one, so a branch's skills are the ones offered. One entry per slug, `.agents` winning a
/// `.claude` it duplicates.
pub async fn skills(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    topic: Option<&str>,
) -> Result<Vec<ProjectSkillRow>, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    let cwd = chat_cwd(s, &project, topic).await?;
    if !cwd.is_dir() {
        return Err(ServiceError::Conflict {
            message: "the project folder is missing".into(),
            code: Some("project_unavailable".into()),
            data: None,
        });
    }

    let found = crate::skill::scopes::SkillScopes::project(&cwd).discover();
    Ok(found
        .skills
        .into_iter()
        .map(|scoped| ProjectSkillRow {
            scope: scoped.scope.label().to_string(),
            folder: scoped.folder(),
            user_invocable: scoped.skill.user_invocable,
            argument_hint: scoped.skill.argument_hint.clone(),
            name: scoped.skill.name,
            description: scoped.skill.description,
        })
        .collect())
}

// ---------------------------------------------------------------------------------------------
// Git (`specs/041-project-coding-workspace` contracts/http-api.md §4)
// ---------------------------------------------------------------------------------------------

fn git_refusal(err: crate::error::VizierError) -> ServiceError {
    let text = err.0;
    let (code, message) = text.split_once(':').unwrap_or(("git_failed", &text));
    let status = match code {
        "git_unavailable" => 409,
        "local_changes" | "branch_exists" | "branch_checked_out" | "branch_not_merged" => 409,
        _ => 500,
    };
    let code = Some(code.trim().to_string());
    match status {
        409 => ServiceError::Conflict {
            message: message.trim().to_string(),
            code,
            data: None,
        },
        422 => invalid_code(code.as_deref().unwrap_or("invalid"), message.trim()),
        _ => ServiceError::Internal(message.trim().to_string()),
    }
}

fn require_git(cwd: &Path) -> Result<crate::projects::git::Git, ServiceError> {
    if !cwd.is_dir() {
        return Err(ServiceError::Conflict {
            message: "the project folder is missing".into(),
            code: Some("project_unavailable".into()),
            data: None,
        });
    }
    Ok(crate::projects::git::Git::new(cwd))
}

async fn ensure_git(git: &crate::projects::git::Git) -> Result<(), ServiceError> {
    match crate::projects::git::probe(&git.dir).await {
        Some(_) => Ok(()),
        None => Err(ServiceError::Conflict {
            message: "git is not available in this folder".into(),
            code: Some("git_unavailable".into()),
            data: None,
        }),
    }
}

/// The header's branch and change count. `available: false` when git is missing or the folder
/// is not a repository (FR-024), never an error.
pub async fn git_summary(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    topic: Option<&str>,
) -> Result<crate::projects::git::GitSummary, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    let cwd = chat_cwd(s, &project, topic).await?;
    if !cwd.is_dir() {
        return Ok(crate::projects::git::GitSummary::unavailable());
    }
    let git = crate::projects::git::Git::new(&cwd);
    if crate::projects::git::probe(&cwd).await.is_none() {
        return Ok(crate::projects::git::GitSummary::unavailable());
    }
    Ok(git.summary().await)
}

pub async fn git_changes(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    topic: Option<&str>,
) -> Result<Vec<crate::projects::git::GitChange>, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    let cwd = chat_cwd(s, &project, topic).await?;
    let git = require_git(&cwd)?;
    ensure_git(&git).await?;
    git.changes().await.map_err(git_refusal)
}

pub async fn git_file_diff(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    topic: Option<&str>,
    path: &str,
) -> Result<crate::projects::git::GitFileDiff, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    let cwd = chat_cwd(s, &project, topic).await?;
    let git = require_git(&cwd)?;
    ensure_git(&git).await?;
    let resolved = crate::projects::path::resolve(&cwd, path, crate::projects::path::Purpose::Existing)
        .map_err(|err| invalid_code("invalid_path", err.0))?;
    let shown = crate::projects::path::display_rel(&cwd, &resolved);
    git.file_diff(&shown, false).await.map_err(git_refusal)
}

pub async fn git_branches(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    topic: Option<&str>,
) -> Result<Vec<crate::projects::git::GitBranch>, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    let cwd = chat_cwd(s, &project, topic).await?;
    let git = require_git(&cwd)?;
    ensure_git(&git).await?;
    let mut branches = git.branches().await.map_err(git_refusal)?;
    // Name the worktree a branch is checked out in, when it is one of this project's.
    let worktrees = s
        .deps()
        .storage
        .worktrees(&project.id)
        .await
        .map_err(internal)?;
    for branch in &mut branches {
        if let Some(holder) = &mut branch.checked_out_in
            && let Some(worktree) = worktrees.iter().find(|worktree| worktree.name == holder.name)
        {
            holder.worktree_id = Some(worktree.id.clone());
        }
    }
    Ok(branches)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SwitchBranchRequest {
    #[serde(default)]
    pub topic: Option<String>,
    pub branch: String,
    #[serde(default)]
    pub create: Option<bool>,
}

/// Switch the folder to another branch, or create one from the current branch. A switch in the
/// main folder needs managing the project; one inside a worktree needs using it (FR-034).
pub async fn git_switch(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    request: SwitchBranchRequest,
) -> Result<crate::projects::git::GitSummary, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    let branch = request.branch.trim();
    if branch.is_empty() {
        return Err(invalid_code("invalid_branch", "a branch name is required"));
    }
    let worktree = match request.topic.as_deref() {
        Some(topic) => s
            .deps()
            .storage
            .worktree_by_topic(&project.id, topic)
            .await
            .map_err(internal)?,
        None => None,
    };
    if let Some(worktree) = &worktree {
        // One's own worktree needs only use; someone else's does not exist for this caller's
        // chats in the first place.
        if worktree.owner_name != who.username && !access::can_manage(Caller::from(who), &project) {
            return Err(ServiceError::Forbidden("Access denied".into()));
        }
        let pending = s.deps().storage.pending_turns(agent_id).await.map_err(internal)?;
        let slug = VizierChannelId::Project(worktree.owner_name.clone(), project.id.clone()).to_slug();
        if pending.iter().any(|turn| {
            turn.session.1.to_slug() == slug
                && turn.session.2.as_deref() == Some(worktree.topic.as_str())
        }) {
            return Err(ServiceError::Conflict {
                message: "a turn is running in this chat; wait for it to finish".into(),
                code: Some("turn_active".into()),
                data: None,
            });
        }
    } else {
        if !access::can_manage(Caller::from(who), &project) {
            return Err(ServiceError::Forbidden(
                "switching the project's own folder needs managing the project".into(),
            ));
        }
        if s.deps()
            .storage
            .project_has_main_turn(agent_id, &project.id)
            .await
            .map_err(internal)?
        {
            return Err(ServiceError::Conflict {
                message: "a turn is running in this project; wait for it to finish".into(),
                code: Some("turn_active".into()),
                data: None,
            });
        }
    }

    let cwd = match &worktree {
        Some(worktree) => worktree.path.clone(),
        None => project.folder.clone(),
    };
    let git = require_git(&cwd)?;
    ensure_git(&git).await?;
    let create = request.create.unwrap_or(false);
    if !create {
        // A branch checked out in another worktree cannot be picked (FR-023).
        let branches = git.branches().await.map_err(git_refusal)?;
        if let Some(found) = branches.iter().find(|found| found.name == branch)
            && let Some(holder) = &found.checked_out_in
        {
            return Err(ServiceError::Conflict {
                message: format!("branch '{branch}' is checked out in worktree '{}'", holder.name),
                code: Some("branch_checked_out".into()),
                data: None,
            });
        }
    }
    let args: Vec<&str> = if create {
        vec!["switch", "-c", branch]
    } else {
        vec!["switch", branch]
    };
    if let Err(err) = git.run_write(&args).await {
        let mapped = git_refusal(err);
        // `git switch -c` of a name git refuses is a validation failure.
        return Err(match &mapped {
            ServiceError::Conflict { code: Some(code), .. } if code == "branch_exists" => mapped,
            ServiceError::Conflict { code: Some(code), .. } if code == "local_changes" => mapped,
            _ if create => invalid_code("invalid_branch", "git refused that branch name"),
            _ => mapped,
        });
    }
    Ok(git.summary().await)
}

// ---------------------------------------------------------------------------------------------
// Worktrees (data-model §3, contracts/http-api.md §5)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateWorktreeRequest {
    pub name: String,
    pub base: String,
    #[serde(default)]
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateWorktreeResponse {
    pub worktree: WorktreeSummary,
    pub topic: String,
    pub channel_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct WorktreeDetail {
    #[serde(flatten)]
    pub worktree: WorktreeSummary,
    pub base_ref: String,
    pub path: String,
    pub created_at: i64,
    pub changed: usize,
    pub untracked: usize,
    pub branch_merged: bool,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct WorktreeDeleted {
    pub deleted: bool,
    pub branch_deleted: bool,
}

/// A folder and branch name (`data-model.md` §3).
fn validate_worktree_name(name: &str) -> Result<String, ServiceError> {
    let name = name.trim();
    let valid = name.len() <= 64
        && name
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !name.ends_with(".lock")
        && !name.contains("..");
    if !valid {
        return Err(invalid_code(
            "invalid_name",
            "the worktree name must start with a letter or digit and use only A–Z, a–z, 0–9, '.', '_' and '-'",
        ));
    }
    Ok(name.to_string())
}

async fn check_ref_format(git: &crate::projects::git::Git, branch: &str) -> Result<(), ServiceError> {
    git.run_read(&["check-ref-format", "--branch", branch])
        .await
        .map(|_| ())
        .map_err(|_| invalid_code("invalid_branch", format!("'{branch}' is not a valid branch name")))
}

pub async fn create_worktree(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    request: CreateWorktreeRequest,
) -> Result<CreateWorktreeResponse, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    if !project.folder.is_dir() {
        return Err(ServiceError::Conflict {
            message: "the project folder is missing".into(),
            code: Some("project_unavailable".into()),
            data: None,
        });
    }
    let git = crate::projects::git::Git::new(&project.folder);
    ensure_git(&git).await?;
    let name = validate_worktree_name(&request.name)?;
    let branch = match request.branch.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
        Some(branch) => {
            check_ref_format(&git, branch).await?;
            branch.to_string()
        }
        None => name.clone(),
    };
    check_ref_format(&git, &branch).await?;
    let base = request.base.trim();
    if base.is_empty() {
        return Err(invalid_code("unknown_base", "choose the branch to start from"));
    }
    if git.run_read(&["rev-parse", "--verify", &format!("refs/heads/{base}")]).await.is_err() {
        return Err(invalid_code("unknown_base", format!("there is no branch '{base}'")));
    }
    if s.deps()
        .storage
        .worktrees(&project.id)
        .await
        .map_err(internal)?
        .iter()
        .any(|worktree| worktree.name == name)
    {
        return Err(ServiceError::Conflict {
            message: format!("a worktree named '{name}' already exists in this project"),
            code: Some("name_in_use".into()),
            data: None,
        });
    }
    let path = project.folder.join(".vizier").join(agent_id).join(&name);
    if path.exists() {
        return Err(ServiceError::Conflict {
            message: format!("the folder {} already exists", channel::display_path(&path)),
            code: Some("path_exists".into()),
            data: None,
        });
    }
    if git
        .branches()
        .await
        .map_err(git_refusal)?
        .iter()
        .any(|found| found.name == branch)
    {
        return Err(ServiceError::Conflict {
            message: format!("branch '{branch}' already exists"),
            code: Some("branch_exists".into()),
            data: None,
        });
    }
    crate::projects::git::worktree::ensure_excluded(&git)
        .await
        .map_err(git_refusal)?;
    let topic: TopicId = nanoid::nanoid!(PROJECT_ID_LEN);
    let path_text = path.to_string_lossy().into_owned();
    git.run_write(&["worktree", "add", "-b", &branch, &path_text, base])
        .await
        .map_err(git_refusal)?;

    let canonical = std::fs::canonicalize(&path).unwrap_or(path.clone());
    let worktree = Worktree {
        id: nanoid::nanoid!(PROJECT_ID_LEN),
        project_id: project.id.clone(),
        agent_id: agent_id.to_string(),
        owner_id: who.user_id.clone(),
        owner_name: who.username.clone(),
        topic: topic.clone(),
        name: name.clone(),
        branch: branch.clone(),
        base_ref: base.to_string(),
        path: canonical,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let detail = VizierSessionDetail {
        agent_id: agent_id.to_string(),
        channel: VizierChannelId::Project(who.username.clone(), project.id.clone()),
        topic: Some(topic.clone()),
        title: name.clone(),
        is_thinking: false,
    };
    if let Err(err) = s.deps().storage.insert_worktree(&worktree, detail).await {
        // Roll the folder back so a failed row never leaves a half-created worktree.
        let _ = git.run_write(&["worktree", "remove", "--force", &path_text]).await;
        return Err(internal(err));
    }
    Ok(CreateWorktreeResponse {
        worktree: WorktreeSummary::of(&worktree),
        topic,
        channel_id: channel::channel_id(&project.id),
    })
}

pub async fn list_worktrees(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
) -> Result<Vec<WorktreeSummary>, ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    Ok(s.deps()
        .storage
        .worktrees(&project.id)
        .await
        .map_err(internal)?
        .iter()
        .map(WorktreeSummary::of)
        .collect())
}

async fn worktree_for_read(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    id: &str,
) -> Result<(Project, Worktree), ServiceError> {
    let project = project_for_read(s, who, agent_id, project_id).await?;
    let worktree = s
        .deps()
        .storage
        .worktree(&project.id, id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServiceError::NotFound("worktree not found".into()))?;
    Ok((project, worktree))
}

pub async fn worktree_detail(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    id: &str,
) -> Result<WorktreeDetail, ServiceError> {
    let (_, worktree) = worktree_for_read(s, who, agent_id, project_id, id).await?;
    let available = worktree.path.is_dir();
    let mut changed = 0usize;
    let mut untracked = 0usize;
    let mut branch_merged = false;
    if available {
        let git = crate::projects::git::Git::new(&worktree.path);
        if let Ok(changes) = git.changes().await {
            untracked = changes
                .iter()
                .filter(|change| change.kind == crate::projects::git::GitChangeKind::Untracked)
                .count();
            changed = changes.len() - untracked;
        }
        if let Ok(output) = git
            .run_read(&["merge-base", "--is-ancestor", &worktree.branch, &worktree.base_ref])
            .await
        {
            branch_merged = output.trim().is_empty();
        }
    }
    Ok(WorktreeDetail {
        worktree: WorktreeSummary::of(&worktree),
        base_ref: worktree.base_ref.clone(),
        path: channel::display_path(&worktree.path),
        created_at: worktree.created_at,
        changed,
        untracked,
        branch_merged,
    })
}

pub async fn delete_worktree(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    project_id: &str,
    id: &str,
    force: bool,
    delete_branch: bool,
) -> Result<WorktreeDeleted, ServiceError> {
    let (project, worktree) = worktree_for_read(s, who, agent_id, project_id, id).await?;
    let own = worktree.owner_name == who.username;
    if !own && !access::can_manage(Caller::from(who), &project) {
        return Err(ServiceError::Forbidden(
            "deleting another person's worktree chat needs managing the project".into(),
        ));
    }
    // Refused while a turn is running in it (FR-030).
    let slug = VizierChannelId::Project(worktree.owner_name.clone(), project.id.clone()).to_slug();
    let pending = s.deps().storage.pending_turns(agent_id).await.map_err(internal)?;
    if pending.iter().any(|turn| {
        turn.session.1.to_slug() == slug
            && turn.session.2.as_deref() == Some(worktree.topic.as_str())
    }) {
        return Err(ServiceError::Conflict {
            message: "a turn is running in this chat; wait for it to finish".into(),
            code: Some("turn_active".into()),
            data: None,
        });
    }

    let git = crate::projects::git::Git::new(&project.folder);
    let available = worktree.path.is_dir();
    if available {
        if !force
            && let Ok(changes) = crate::projects::git::Git::new(&worktree.path).changes().await
        {
            let untracked = changes
                .iter()
                .filter(|change| change.kind == crate::projects::git::GitChangeKind::Untracked)
                .count();
            let changed = changes.len() - untracked;
            if changed + untracked > 0 {
                return Err(ServiceError::Conflict {
                    message: format!(
                        "{changed} changed and {untracked} untracked files would be lost"
                    ),
                    code: Some("worktree_dirty".into()),
                    data: Some(serde_json::json!({ "changed": changed, "untracked": untracked })),
                });
            }
        }
        let path = worktree.path.to_string_lossy().into_owned();
        let args: Vec<&str> = if force {
            vec!["worktree", "remove", "--force", &path]
        } else {
            vec!["worktree", "remove", &path]
        };
        git.run_write(&args).await.map_err(git_refusal)?;
    }
    let _ = git.run_write(&["worktree", "prune"]).await;

    let mut branch_deleted = false;
    if delete_branch {
        let args: Vec<&str> = if force {
            vec!["branch", "-D", &worktree.branch]
        } else {
            vec!["branch", "-d", &worktree.branch]
        };
        branch_deleted = git.run_write(&args).await.is_ok();
    }

    // The chat goes through the existing conversation deletion, so history, files, turns and
    // jobs are cleaned by the code that already owns them (R3).
    let session = VizierSession(
        agent_id.to_string(),
        VizierChannelId::Project(worktree.owner_name.clone(), project.id.clone()),
        Some(worktree.topic.clone()),
    );
    s.deps()
        .session_files
        .delete_conversation(&session)
        .await
        .map_err(|err| ServiceError::Internal(err.to_string()))?;
    s.deps()
        .project_ledger
        .forget_session(&session.to_slug());
    s.deps()
        .storage
        .delete_worktree(&worktree.id)
        .await
        .map_err(internal)?;
    Ok(WorktreeDeleted { deleted: true, branch_deleted })
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

    fn write_skill(root: &std::path::Path, slug: &str, description: &str) {
        let dir = root.join(".agents/skills").join(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {slug}\ndescription: {description}\n---\n\nbody\n"),
        )
        .unwrap();
    }

    /// The composer's `/` menu reads this: the chat's own folder, which is the worktree when the
    /// chat is bound to one, so a branch's skills are the ones offered (FR-035).
    #[tokio::test]
    async fn a_chats_skills_come_from_its_own_folder() {
        let (services, who, _dir) = setup().await;
        let root = folder();
        write_skill(root.path(), "root-only", "the project root");
        let opened = open(&services, &who, AuthMethod::Desktop, "dp-a", request(&root))
            .await
            .unwrap();
        let project_id = opened.project.id.clone();

        let listed = skills(&services, &who, "dp-a", &project_id, None).await.unwrap();
        let names: Vec<String> = listed.iter().map(|row| row.name.clone()).collect();
        assert_eq!(names, ["root-only"]);
        assert!(listed[0].user_invocable, "the default is invocable");
        assert_eq!(listed[0].scope, "project");

        // A worktree of the same project carries its own branch's skills, not the root's.
        let worktree_dir = root.path().join(".vizier/dp-a/fix");
        std::fs::create_dir_all(&worktree_dir).unwrap();
        write_skill(&worktree_dir, "branch-only", "the worktree");
        let detail = crate::schema::VizierSessionDetail {
            agent_id: "dp-a".into(),
            channel: VizierChannelId::Project("alice".into(), project_id.clone()),
            topic: Some("t1".into()),
            title: "fix".into(),
            is_thinking: false,
        };
        let worktree = crate::schema::Worktree {
            id: "w1".into(),
            project_id: project_id.clone(),
            agent_id: "dp-a".into(),
            owner_id: who.user_id.clone(),
            owner_name: "alice".into(),
            topic: "t1".into(),
            name: "fix".into(),
            branch: "fix".into(),
            base_ref: "master".into(),
            path: worktree_dir.clone(),
            created_at: 0,
        };
        services
            .deps()
            .storage
            .insert_worktree(&worktree, detail)
            .await
            .unwrap();

        let in_worktree = skills(&services, &who, "dp-a", &project_id, Some("t1")).await.unwrap();
        let names: Vec<String> = in_worktree.iter().map(|row| row.name.clone()).collect();
        assert_eq!(names, ["branch-only"], "the worktree's checkout, not the project root");
    }

    #[tokio::test]
    async fn a_missing_folder_is_a_conflict_not_an_empty_list() {
        let (services, who, _dir) = setup().await;
        let root = folder();
        let opened = open(&services, &who, AuthMethod::Desktop, "dp-a", request(&root))
            .await
            .unwrap();
        let path = std::fs::canonicalize(root.path()).unwrap();
        drop(root);
        assert!(!path.is_dir());
        let err = skills(&services, &who, "dp-a", &opened.project.id, None)
            .await
            .unwrap_err();
        assert_eq!(err.code(), Some("project_unavailable"));
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

    fn git_repo() -> Option<tempfile::TempDir> {
        let ok = std::process::Command::new("git")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success());
        if !ok {
            return None;
        }
        let dir = tempfile::tempdir().unwrap();
        for args in [
            vec!["init", "-q", "-b", "master"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "user.name", "T"],
        ] {
            assert!(std::process::Command::new("git").current_dir(dir.path()).args(&args).status().unwrap().success());
        }
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        assert!(std::process::Command::new("git").current_dir(dir.path()).args(["add", "-A"]).status().unwrap().success());
        assert!(std::process::Command::new("git").current_dir(dir.path()).args(["commit", "-qm", "init"]).status().unwrap().success());
        Some(dir)
    }

    #[tokio::test]
    async fn a_worktree_is_created_on_its_own_branch_and_removed_with_its_chat() {
        let Some(folder) = git_repo() else { return };
        let (services, who, _dir) = setup().await;
        let opened = open(&services, &who, AuthMethod::Desktop, "dp-a", request(&folder))
            .await
            .unwrap();
        let created = create_worktree(
            &services,
            &who,
            "dp-a",
            &opened.project.id,
            CreateWorktreeRequest {
                name: "fix-ps".into(),
                base: "master".into(),
                branch: None,
            },
        )
        .await
        .unwrap();
        let path = folder.path().join(".vizier/dp-a/fix-ps");
        assert!(path.is_dir());
        assert_eq!(created.worktree.branch, "fix-ps");
        assert_eq!(list_worktrees(&services, &who, "dp-a", &opened.project.id).await.unwrap().len(), 1);
        let detail = worktree_detail(&services, &who, "dp-a", &opened.project.id, &created.worktree.id)
            .await
            .unwrap();
        assert!(detail.worktree.available);
        assert_eq!((detail.changed, detail.untracked), (0, 0));
        assert!(detail.branch_merged);

        delete_worktree(
            &services,
            &who,
            "dp-a",
            &opened.project.id,
            &created.worktree.id,
            false,
            true,
        )
        .await
        .unwrap();
        assert!(!path.exists());
        assert!(list_worktrees(&services, &who, "dp-a", &opened.project.id).await.unwrap().is_empty());
    }

}
