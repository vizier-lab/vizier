//! The per-turn project context (`specs/041-project-coding-workspace` R10/R11): resolved once
//! when a turn starts, then used by the system prompt, the coding tools, mentions and the shell.
//!
//! A `Project` session resolves the project, the topic's worktree (its `cwd`), the saved mode
//! and the git probe. A `Subagent` piece launched from a project chat carries a
//! `project_scope` on its request and resolves the same project under the launching turn's
//! mode. Every other session kind gets nothing.

use std::path::PathBuf;
use std::sync::Arc;

use crate::{
    access::Caller,
    dependencies::VizierDependencies,
    projects::{
        access,
        git::{GitInfo, probe},
        instructions,
    },
    schema::{
        ProjectMode, VizierChannelId, VizierRequest, VizierSession, WorktreeRef,
    },
    storage::{agent::AgentStorage, project::ProjectStorage, user::UserStorage},
};

#[derive(Debug, Clone)]
pub struct ProjectTurnContext {
    pub project_id: String,
    pub name: String,
    /// Canonical absolute project root.
    pub folder: PathBuf,
    /// The folder this turn acts in: the worktree for a worktree chat, else the root.
    pub cwd: PathBuf,
    /// The worktree this chat is bound to, when it has one.
    pub worktree: Option<WorktreeRef>,
    /// Read once at turn start, so a switch applies from the next turn (FR-013).
    pub mode: ProjectMode,
    /// The enclosing repository, when git is present and `cwd` is inside one.
    pub git: Option<GitInfo>,
    /// `cwd.is_dir()` at read time.
    pub available: bool,
    pub shell_enabled: bool,
    pub instructions: Option<String>,
    pub instructions_truncated: bool,
    /// A subagent piece launched from a project chat, not the chat itself.
    pub is_piece: bool,
    /// The stale-read guard, shared with the tools that read and write project files.
    pub ledger: Arc<crate::projects::ledger::ProjectReadLedger>,
}

impl ProjectTurnContext {
    /// Whether the workspace shell may be offered this turn: the folder is there, the project's
    /// switch is on, and this turn is in Build mode.
    pub fn shell_offered(&self) -> bool {
        self.available && self.shell_enabled && self.mode == ProjectMode::Build
    }

    /// Whether `propose_plan` may be offered: only the person's own chat may end a planning
    /// turn, never a piece reporting to its launcher.
    pub fn plan_offered(&self) -> bool {
        !self.is_piece && self.mode == ProjectMode::Plan
    }
}

/// What a `Project` or scoped `Subagent` session needs, before storage is read.
struct Scope {
    username: String,
    project_id: String,
    topic: String,
    mode: Option<ProjectMode>,
    is_piece: bool,
}

fn scope_of(session: &VizierSession, request: &VizierRequest) -> Option<Scope> {
    match (&session.1, &request.project_scope) {
        (VizierChannelId::Project(user, project_id), _) => Some(Scope {
            username: user.clone(),
            project_id: project_id.clone(),
            topic: session.2.clone().unwrap_or_default(),
            mode: None,
            is_piece: false,
        }),
        (VizierChannelId::Subagent, Some(scope)) => Some(Scope {
            username: scope.owner.clone(),
            project_id: scope.project_id.clone(),
            topic: scope.topic.clone(),
            mode: Some(scope.mode),
            is_piece: true,
        }),
        _ => None,
    }
}

/// The project a turn works in, when it is a project chat or a piece launched from one and its
/// owner may still use it. A removed project, a revoked permission or any other session kind is
/// `None`.
pub async fn for_turn(
    deps: &VizierDependencies,
    session: &VizierSession,
    request: &VizierRequest,
) -> Option<Arc<ProjectTurnContext>> {
    let scope = scope_of(session, request)?;
    let project = deps
        .storage
        .get_project(&session.0, &scope.project_id)
        .await
        .ok()
        .flatten()?;
    if project.removed_at.is_some() {
        return None;
    }

    // Re-read the session owner's *current* permission on every turn: revocation disables the
    // project section and the tools on the next turn, even for an internally woken report or a
    // resumed piece (FR-007a).
    let user = deps.storage.get_user(&scope.username).await.ok().flatten()?;
    let role = deps.storage.get_role(&user.role_id).await.ok().flatten()?;
    let agent = deps.storage.get_agent(&session.0).await.ok().flatten()?;
    let caller = Caller {
        user_id: &user.user_id,
        is_system: role.is_system,
        permissions: &role.permissions,
    };
    if !access::can_use(caller, &agent) {
        return None;
    }

    let worktree = deps
        .storage
        .worktree_by_topic(&project.id, &scope.topic)
        .await
        .ok()
        .flatten();
    let cwd = match &worktree {
        Some(worktree) => worktree.path.clone(),
        None => project.folder.clone(),
    };
    let mode = match scope.mode {
        Some(mode) => mode,
        None => deps
            .storage
            .project_chat_mode(&session.0, &session.1.to_slug(), &scope.topic)
            .await
            .unwrap_or_default(),
    };
    let available = cwd.is_dir();
    let instructions = match available {
        true => instructions::read(&cwd),
        false => None,
    };
    let git = match available {
        true => probe(&cwd).await,
        false => None,
    };

    Some(Arc::new(ProjectTurnContext {
        project_id: project.id,
        name: project.name,
        folder: project.folder,
        cwd,
        worktree: worktree.as_ref().map(WorktreeRef::from),
        mode,
        git,
        available,
        shell_enabled: project.shell_enabled,
        instructions: instructions.as_ref().map(|i| i.text.clone()),
        instructions_truncated: instructions.is_some_and(|i| i.truncated),
        is_piece: scope.is_piece,
        ledger: deps.project_ledger.clone(),
    }))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::{
        config::VizierConfig,
        schema::{DreamStage, Project, ProjectScope},
        storage::project::ProjectStorage,
    };

    fn project(agent: &str, owner: &str, folder: &std::path::Path) -> Project {
        Project {
            id: "p3xY9aQ2mZ".into(),
            agent_id: agent.into(),
            name: "alpha".into(),
            folder: std::fs::canonicalize(folder).unwrap(),
            owner_id: owner.into(),
            shell_enabled: true,
            created_at: Utc::now().timestamp_millis(),
            removed_at: None,
        }
    }

    async fn setup() -> (VizierDependencies, tempfile::TempDir, tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let deps = VizierDependencies::new(VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        let role = deps
            .storage
            .create_role(
                "projects",
                vec!["all_agents:view".into(), access::USE_OWNED.into()],
                false,
            )
            .await
            .unwrap();
        let user = deps
            .storage
            .create_user("alice", "hash", &role.role_id)
            .await
            .unwrap();
        let mut agent = crate::tests_support::agent_config("dp-a");
        agent.owner_id = Some(user.user_id.clone());
        deps.storage.create_agent("dp-a", &agent).await.unwrap();
        let folder = tempfile::tempdir().unwrap();
        std::fs::write(folder.path().join("AGENTS.md"), "Always answer in French.").unwrap();
        deps.storage
            .open_project(project("dp-a", &user.user_id, folder.path()))
            .await
            .unwrap();
        (deps, dir, folder, user.user_id)
    }

    fn project_session(agent: &str, owner: &str, topic: &str) -> VizierSession {
        VizierSession(
            agent.into(),
            VizierChannelId::Project(owner.into(), "p3xY9aQ2mZ".into()),
            Some(topic.to_string()),
        )
    }

    fn chat() -> VizierRequest {
        VizierRequest {
            content: crate::schema::VizierRequestContent::Chat("hi".into()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn a_project_session_resolves_its_folder_mode_and_instructions() {
        let (deps, _dir, _folder, _uid) = setup().await;
        let ctx = for_turn(&deps, &project_session("dp-a", "alice", "t1"), &chat())
            .await
            .expect("project context");
        assert_eq!(ctx.name, "alpha");
        assert!(ctx.available);
        assert_eq!(ctx.mode, ProjectMode::Plan);
        assert!(!ctx.shell_offered(), "Plan mode never offers the shell");
        assert!(!ctx.is_piece);
        assert_eq!(ctx.cwd, ctx.folder);
        assert_eq!(ctx.instructions.as_deref(), Some("Always answer in French."));
    }

    #[tokio::test]
    async fn build_mode_and_the_shell_switch_control_the_shell() {
        let (deps, _dir, _folder, _uid) = setup().await;
        let session = project_session("dp-a", "alice", "t1");
        deps.storage
            .set_project_chat_mode("dp-a", &session.1.to_slug(), "t1", ProjectMode::Build)
            .await
            .unwrap();
        let ctx = for_turn(&deps, &session, &chat()).await.unwrap();
        assert_eq!(ctx.mode, ProjectMode::Build);
        assert!(ctx.shell_offered());

        deps.storage
            .set_project_shell("p3xY9aQ2mZ", false)
            .await
            .unwrap();
        let ctx = for_turn(&deps, &session, &chat()).await.unwrap();
        assert!(!ctx.shell_offered(), "the project's shell switch still applies");
    }

    #[tokio::test]
    async fn every_non_project_channel_is_none() {
        let (deps, _dir, _folder, _uid) = setup().await;
        let agent = "dp-a".to_string();
        let ordinary = VizierSession(
            agent.clone(),
            VizierChannelId::HTTP("alice".into(), "vizier-webui".into()),
            Some("t1".into()),
        );
        let task = VizierSession(
            agent.clone(),
            VizierChannelId::Task("tick".into(), Utc::now()),
            None,
        );
        let subagent = VizierSession(agent.clone(), VizierChannelId::Subagent, None);
        let inter_agent = VizierSession(
            agent.clone(),
            VizierChannelId::InterAgent(vec!["dp-a".into(), "dp-b".into()]),
            None,
        );
        let system = VizierSession(agent.clone(), VizierChannelId::System, None);
        let dream = VizierSession(
            agent.clone(),
            VizierChannelId::Dream(
                Box::new(project_session("dp-a", "alice", "t1")),
                DreamStage::Extraction,
            ),
            None,
        );
        for session in [ordinary, task, subagent, inter_agent, system, dream] {
            assert!(for_turn(&deps, &session, &chat()).await.is_none(), "{session:?}");
        }
    }

    #[tokio::test]
    async fn only_subagents_with_a_scope_inherit_the_project() {
        let (deps, _dir, _folder, _uid) = setup().await;
        let piece = VizierSession("dp-a".into(), VizierChannelId::Subagent, Some("piece".into()));
        // No scope: a plain subagent.
        assert!(for_turn(&deps, &piece, &chat()).await.is_none());

        let mut request = chat();
        request.project_scope = Some(ProjectScope {
            project_id: "p3xY9aQ2mZ".into(),
            owner: "alice".into(),
            topic: "t1".into(),
            mode: ProjectMode::Build,
        });
        let ctx = for_turn(&deps, &piece, &request).await.expect("scoped piece");
        assert!(ctx.is_piece);
        assert_eq!(ctx.mode, ProjectMode::Build, "the launching turn's mode");
        assert_eq!(ctx.cwd, ctx.folder);
        assert!(!ctx.plan_offered(), "a piece never gets propose_plan");
    }

    #[tokio::test]
    async fn a_scoped_piece_resolves_the_launching_chats_worktree() {
        let (deps, _dir, folder, uid) = setup().await;
        let wt_dir = folder.path().join(".vizier/dp-a/fix");
        std::fs::create_dir_all(&wt_dir).unwrap();
        let worktree = crate::schema::Worktree {
            id: "w1".into(),
            project_id: "p3xY9aQ2mZ".into(),
            agent_id: "dp-a".into(),
            owner_id: uid,
            owner_name: "alice".into(),
            topic: "t1".into(),
            name: "fix".into(),
            branch: "fix".into(),
            base_ref: "master".into(),
            path: std::fs::canonicalize(&wt_dir).unwrap(),
            created_at: 0,
        };
        let detail = crate::schema::VizierSessionDetail {
            agent_id: "dp-a".into(),
            channel: VizierChannelId::Project("alice".into(), "p3xY9aQ2mZ".into()),
            topic: Some("t1".into()),
            title: "fix".into(),
            is_thinking: false,
        };
        deps.storage.insert_worktree(&worktree, detail).await.unwrap();

        let piece = VizierSession("dp-a".into(), VizierChannelId::Subagent, Some("piece".into()));
        let mut request = chat();
        request.project_scope = Some(ProjectScope {
            project_id: "p3xY9aQ2mZ".into(),
            owner: "alice".into(),
            topic: "t1".into(),
            mode: ProjectMode::Plan,
        });
        let ctx = for_turn(&deps, &piece, &request).await.expect("scoped piece");
        assert_eq!(ctx.cwd, std::fs::canonicalize(&wt_dir).unwrap());
        assert_eq!(ctx.worktree.as_ref().map(|w| w.name.as_str()), Some("fix"));
    }

    #[tokio::test]
    async fn a_revoked_permission_removes_the_context() {
        let (deps, _dir, _folder, _uid) = setup().await;
        let session = project_session("dp-a", "alice", "t1");
        assert!(for_turn(&deps, &session, &chat()).await.is_some());
        let role = deps
            .storage
            .get_user("alice")
            .await
            .unwrap()
            .map(|user| user.role_id)
            .unwrap();
        deps.storage
            .update_role(&role, "projects", vec!["all_agents:view".into()])
            .await
            .unwrap();
        assert!(for_turn(&deps, &session, &chat()).await.is_none());
    }

    #[tokio::test]
    async fn a_removed_project_resolves_to_none_for_turns() {
        let (deps, _dir, _folder, _uid) = setup().await;
        deps.storage
            .remove_project("p3xY9aQ2mZ", Utc::now().timestamp_millis())
            .await
            .unwrap();
        assert!(
            for_turn(&deps, &project_session("dp-a", "alice", "t1"), &chat())
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn a_missing_folder_is_unavailable_and_has_no_instructions() {
        let (deps, _dir, folder, _uid) = setup().await;
        let path = std::fs::canonicalize(folder.path()).unwrap();
        drop(folder);
        assert!(!path.is_dir());
        let ctx = for_turn(&deps, &project_session("dp-a", "alice", "t1"), &chat())
            .await
            .expect("available: false is still context");
        assert!(!ctx.available);
        assert!(!ctx.shell_offered());
        assert!(ctx.instructions.is_none());
        assert!(ctx.git.is_none());
    }
}
