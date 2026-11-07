//! Long-lived projects (`specs/040-projects-mode/data-model.md` §2): the folder an agent works
//! in, the shell switch, the owner and the soft-removed state. Storage is deliberately unaware
//! of permissions and session identity; the service layer owns both.

use anyhow::Result;

use crate::{
    schema::{
        Project, ProjectMode, ProjectSessionRow, ServerOverride, VizierSessionDetail, Worktree,
    },
    storage::VizierStorage,
};

#[async_trait::async_trait]
pub trait ProjectStorage {
    /// An agent's projects, newest first. Removed rows are excluded unless `include_removed`.
    async fn list_projects(&self, agent_id: &str, include_removed: bool) -> Result<Vec<Project>>;

    /// One project of this agent, removed or not; `None` when it does not exist here.
    async fn get_project(&self, agent_id: &str, project_id: &str) -> Result<Option<Project>>;

    /// Insert a project, or restore/return the existing row for `(agent_id, folder)`. The bool
    /// is `true` only when a row was inserted (the API's `201`).
    async fn open_project(&self, project: Project) -> Result<(Project, bool)>;

    async fn rename_project(&self, project_id: &str, name: &str) -> Result<()>;

    async fn set_project_shell(&self, project_id: &str, enabled: bool) -> Result<()>;

    /// The language-server switch (`specs/054-project-lsp`).
    async fn set_project_lsp(&self, project_id: &str, enabled: bool) -> Result<()>;

    /// A project's server overrides, by language id.
    async fn lsp_overrides(&self, project_id: &str) -> Result<Vec<(String, ServerOverride)>>;

    /// Save the override for one language; the environment is stored encrypted.
    async fn set_lsp_override(
        &self,
        project_id: &str,
        language: &str,
        server: &ServerOverride,
        updated_by: &str,
        updated_at: i64,
    ) -> Result<()>;

    /// Remove one override. Removing an absent one succeeds.
    async fn delete_lsp_override(&self, project_id: &str, language: &str) -> Result<()>;

    /// Set `removed_at`; nothing else changes.
    async fn remove_project(&self, project_id: &str, removed_at: i64) -> Result<()>;

    /// The sessions of one exact project channel slug in creation order, newest first: a
    /// worktree chat by when the worktree was made, any other by its first message. A brand-new
    /// session that has no message yet was created last, so it sorts last.
    async fn project_sessions(
        &self,
        agent_id: &str,
        channel_slug: &str,
    ) -> Result<Vec<ProjectSessionRow>>;

    /// How many sessions the project has, across every owner (the sidebar's project row).
    async fn project_session_count(&self, agent_id: &str, project_id: &str) -> Result<usize>;

    /// A chat's saved mode; no row means `Plan` (FR-009).
    async fn project_chat_mode(
        &self,
        agent_id: &str,
        channel: &str,
        topic: &str,
    ) -> Result<ProjectMode>;

    /// Upsert a chat's mode. Written only by `PUT …/sessions/{topic}/mode`.
    async fn set_project_chat_mode(
        &self,
        agent_id: &str,
        channel: &str,
        topic: &str,
        mode: ProjectMode,
    ) -> Result<()>;

    /// Insert a worktree row and its chat's `session_detail` in one transaction, so a worktree
    /// never exists without the chat it belongs to.
    async fn insert_worktree(
        &self,
        worktree: &Worktree,
        detail: VizierSessionDetail,
    ) -> Result<()>;

    async fn worktree(&self, project_id: &str, id: &str) -> Result<Option<Worktree>>;

    async fn worktree_by_topic(
        &self,
        project_id: &str,
        topic: &str,
    ) -> Result<Option<Worktree>>;

    async fn worktrees(&self, project_id: &str) -> Result<Vec<Worktree>>;

    async fn delete_worktree(&self, id: &str) -> Result<()>;

    /// Whether a turn is pending in the project's main folder (R14): any project chat of this
    /// agent that is not bound to a worktree.
    async fn project_has_main_turn(&self, agent_id: &str, project_id: &str) -> Result<bool>;
}

#[async_trait::async_trait]
impl ProjectStorage for VizierStorage {
    async fn list_projects(&self, agent_id: &str, include_removed: bool) -> Result<Vec<Project>> {
        self.0.list_projects(agent_id, include_removed).await
    }

    async fn get_project(&self, agent_id: &str, project_id: &str) -> Result<Option<Project>> {
        self.0.get_project(agent_id, project_id).await
    }

    async fn open_project(&self, project: Project) -> Result<(Project, bool)> {
        self.0.open_project(project).await
    }

    async fn rename_project(&self, project_id: &str, name: &str) -> Result<()> {
        self.0.rename_project(project_id, name).await
    }

    async fn set_project_shell(&self, project_id: &str, enabled: bool) -> Result<()> {
        self.0.set_project_shell(project_id, enabled).await
    }

    async fn set_project_lsp(&self, project_id: &str, enabled: bool) -> Result<()> {
        self.0.set_project_lsp(project_id, enabled).await
    }

    async fn lsp_overrides(&self, project_id: &str) -> Result<Vec<(String, ServerOverride)>> {
        self.0.lsp_overrides(project_id).await
    }

    async fn set_lsp_override(
        &self,
        project_id: &str,
        language: &str,
        server: &ServerOverride,
        updated_by: &str,
        updated_at: i64,
    ) -> Result<()> {
        self.0
            .set_lsp_override(project_id, language, server, updated_by, updated_at)
            .await
    }

    async fn delete_lsp_override(&self, project_id: &str, language: &str) -> Result<()> {
        self.0.delete_lsp_override(project_id, language).await
    }

    async fn remove_project(&self, project_id: &str, removed_at: i64) -> Result<()> {
        self.0.remove_project(project_id, removed_at).await
    }

    async fn project_sessions(
        &self,
        agent_id: &str,
        channel_slug: &str,
    ) -> Result<Vec<ProjectSessionRow>> {
        self.0.project_sessions(agent_id, channel_slug).await
    }

    async fn project_session_count(&self, agent_id: &str, project_id: &str) -> Result<usize> {
        self.0.project_session_count(agent_id, project_id).await
    }

    async fn project_chat_mode(
        &self,
        agent_id: &str,
        channel: &str,
        topic: &str,
    ) -> Result<ProjectMode> {
        self.0.project_chat_mode(agent_id, channel, topic).await
    }

    async fn set_project_chat_mode(
        &self,
        agent_id: &str,
        channel: &str,
        topic: &str,
        mode: ProjectMode,
    ) -> Result<()> {
        self.0
            .set_project_chat_mode(agent_id, channel, topic, mode)
            .await
    }

    async fn insert_worktree(
        &self,
        worktree: &Worktree,
        detail: VizierSessionDetail,
    ) -> Result<()> {
        self.0.insert_worktree(worktree, detail).await
    }

    async fn worktree(&self, project_id: &str, id: &str) -> Result<Option<Worktree>> {
        self.0.worktree(project_id, id).await
    }

    async fn worktree_by_topic(&self, project_id: &str, topic: &str) -> Result<Option<Worktree>> {
        self.0.worktree_by_topic(project_id, topic).await
    }

    async fn worktrees(&self, project_id: &str) -> Result<Vec<Worktree>> {
        self.0.worktrees(project_id).await
    }

    async fn delete_worktree(&self, id: &str) -> Result<()> {
        self.0.delete_worktree(id).await
    }

    async fn project_has_main_turn(&self, agent_id: &str, project_id: &str) -> Result<bool> {
        self.0.project_has_main_turn(agent_id, project_id).await
    }
}
