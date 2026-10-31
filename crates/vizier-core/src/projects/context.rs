//! The per-turn project context (`specs/040-projects-mode/contracts/session-and-permissions.md`
//! §6): resolved once for a project session, then used by the system prompt and the workspace
//! shell. Every other session kind resolves to `None`.

use std::path::PathBuf;
use std::sync::Arc;

use crate::{
    access::Caller,
    dependencies::VizierDependencies,
    projects::{access, channel, instructions},
    schema::{VizierChannelId, VizierSession},
    storage::{agent::AgentStorage, project::ProjectStorage, user::UserStorage},
};

#[derive(Debug, Clone)]
pub struct ProjectTurnContext {
    pub project_id: String,
    pub name: String,
    /// Canonical absolute path.
    pub folder: PathBuf,
    pub available: bool,
    pub shell_enabled: bool,
    pub instructions: Option<String>,
    pub instructions_truncated: bool,
}

impl ProjectTurnContext {
    /// Whether the workspace shell may be offered this turn: the folder is there and the
    /// project's switch is on.
    pub fn shell_offered(&self) -> bool {
        self.available && self.shell_enabled
    }
}

/// The project a session belongs to, when it is a project session and its owner may still use
/// it. A removed project, a revoked permission or a non-project channel is `None`.
pub async fn for_session(
    deps: &VizierDependencies,
    session: &VizierSession,
) -> Option<Arc<ProjectTurnContext>> {
    let VizierChannelId::HTTP(username, channel_id) = &session.1 else {
        return None;
    };
    let project_id = channel::project_id(channel_id)?;
    let project = deps
        .storage
        .get_project(&session.0, project_id)
        .await
        .ok()
        .flatten()?;
    if project.removed_at.is_some() {
        return None;
    }

    // Re-read the session owner's *current* permission on every turn: revocation disables the
    // project section and the shell on the next turn, even for an internally woken report.
    let user = deps.storage.get_user(username).await.ok().flatten()?;
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

    let available = project.folder.is_dir();
    let instructions = match available {
        true => instructions::read(&project.folder),
        false => None,
    };
    Some(Arc::new(ProjectTurnContext {
        project_id: project.id,
        name: project.name,
        folder: project.folder,
        available,
        shell_enabled: project.shell_enabled,
        instructions: instructions.as_ref().map(|i| i.text.clone()),
        instructions_truncated: instructions.is_some_and(|i| i.truncated),
    }))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::{
        config::VizierConfig,
        schema::{DreamStage, Project},
        storage::{project::ProjectStorage, user::UserStorage},
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
            VizierChannelId::HTTP(owner.into(), channel::channel_id("p3xY9aQ2mZ")),
            Some(topic.to_string()),
        )
    }

    #[tokio::test]
    async fn a_project_session_resolves_its_folder_and_instructions() {
        let (deps, _dir, _folder, _uid) = setup().await;
        let ctx = for_session(&deps, &project_session("dp-a", "alice", "t1"))
            .await
            .expect("project context");
        assert_eq!(ctx.name, "alpha");
        assert!(ctx.available);
        assert!(ctx.shell_offered());
        assert_eq!(ctx.instructions.as_deref(), Some("Always answer in French."));
        assert!(!ctx.instructions_truncated);
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
            assert!(for_session(&deps, &session).await.is_none(), "{session:?}");
        }
    }

    #[tokio::test]
    async fn a_revoked_permission_removes_the_context() {
        let (deps, _dir, _folder, _uid) = setup().await;
        assert!(for_session(&deps, &project_session("dp-a", "alice", "t1")).await.is_some());
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
        assert!(for_session(&deps, &project_session("dp-a", "alice", "t1")).await.is_none());
    }

    #[tokio::test]
    async fn a_removed_project_resolves_to_none_for_turns() {
        let (deps, _dir, _folder, _uid) = setup().await;
        deps.storage
            .remove_project("p3xY9aQ2mZ", Utc::now().timestamp_millis())
            .await
            .unwrap();
        assert!(for_session(&deps, &project_session("dp-a", "alice", "t1")).await.is_none());
    }

    #[tokio::test]
    async fn a_missing_folder_is_unavailable_and_has_no_instructions() {
        let (deps, _dir, folder, _uid) = setup().await;
        let path = std::fs::canonicalize(folder.path()).unwrap();
        drop(folder);
        // The temp folder is gone, so the project is unavailable but the history stays readable.
        assert!(!path.is_dir());
        let ctx = for_session(&deps, &project_session("dp-a", "alice", "t1"))
            .await
            .expect("available: false is still context");
        assert!(!ctx.available);
        assert!(!ctx.shell_offered());
        assert!(ctx.instructions.is_none());
    }
}
