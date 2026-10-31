//! Long-lived projects (`specs/040-projects-mode/data-model.md` §2): the folder an agent works
//! in, the shell switch, the owner and the soft-removed state. Storage is deliberately unaware
//! of permissions and session identity; the service layer owns both.

use anyhow::Result;

use crate::{schema::{Project, ProjectSessionRow}, storage::VizierStorage};

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

    /// Set `removed_at`; nothing else changes.
    async fn remove_project(&self, project_id: &str, removed_at: i64) -> Result<()>;

    /// The sessions of one exact project channel slug, newest activity first, with a brand-new
    /// empty session last.
    async fn project_sessions(
        &self,
        agent_id: &str,
        channel_slug: &str,
    ) -> Result<Vec<ProjectSessionRow>>;

    /// How many sessions the project has, across every owner (the sidebar's project row).
    async fn project_session_count(&self, agent_id: &str, project_id: &str) -> Result<usize>;
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
}
