//! Workspaces, their topics and members (`specs/053-workspace-channel/data-model.md`). Storage
//! is unaware of permissions; a unique-constraint failure comes back as an error that
//! [`crate::storage::sql::is_constraint_violation`] recognises, and the service layer maps it.

use anyhow::Result;

use crate::{
    schema::{MemberKind, Workspace, WorkspaceMember, WorkspaceRole, WorkspaceTopic},
    storage::VizierStorage,
};

#[async_trait::async_trait]
pub trait WorkspaceStorage {
    /// Inserts the Workspace, its first topic and its first owner in one transaction. A name
    /// that differs only in case fails the unique constraint.
    async fn create_workspace(
        &self,
        workspace: &Workspace,
        topic: &WorkspaceTopic,
        owner: &WorkspaceMember,
    ) -> Result<()>;

    async fn get_workspace(&self, id: &str) -> Result<Option<Workspace>>;

    /// By name, ignoring case.
    async fn find_workspace_by_name(&self, name: &str) -> Result<Option<Workspace>>;

    /// The Workspaces a person is a member of, by name, with their role.
    async fn workspaces_of_person(&self, username: &str) -> Result<Vec<(Workspace, WorkspaceRole)>>;

    /// The ids of the Workspaces an agent is a member of.
    async fn workspaces_of_agent(&self, agent_id: &str) -> Result<Vec<String>>;

    /// Every Workspace, for someone who may manage them all.
    async fn list_workspaces(&self) -> Result<Vec<Workspace>>;

    async fn rename_workspace(&self, id: &str, name: &str, now: i64) -> Result<()>;

    /// Deletes the Workspace; topics, members, entries, mentions and read positions cascade.
    async fn delete_workspace(&self, id: &str) -> Result<()>;

    async fn list_topics(&self, workspace_id: &str) -> Result<Vec<WorkspaceTopic>>;

    async fn get_topic(&self, workspace_id: &str, topic_id: &str) -> Result<Option<WorkspaceTopic>>;

    /// A topic by name, ignoring case.
    async fn find_topic_by_name(&self, workspace_id: &str, name: &str) -> Result<Option<WorkspaceTopic>>;

    /// Fails the unique constraint when the name is taken in this Workspace.
    async fn create_topic(&self, topic: &WorkspaceTopic) -> Result<()>;

    async fn rename_topic(&self, workspace_id: &str, topic_id: &str, name: &str) -> Result<()>;

    async fn delete_topic(&self, workspace_id: &str, topic_id: &str) -> Result<()>;

    async fn list_members(&self, workspace_id: &str) -> Result<Vec<WorkspaceMember>>;

    /// By handle, whatever the kind: a handle is unique in a Workspace.
    async fn get_member(&self, workspace_id: &str, handle: &str) -> Result<Option<WorkspaceMember>>;

    /// Fails the unique constraint when the handle is already a member.
    async fn add_member(&self, member: &WorkspaceMember) -> Result<()>;

    async fn set_member_role(
        &self,
        workspace_id: &str,
        handle: &str,
        role: WorkspaceRole,
    ) -> Result<()>;

    async fn remove_member(&self, workspace_id: &str, kind: MemberKind, handle: &str) -> Result<()>;

    /// Removes an agent from every Workspace, when the agent itself is deleted.
    async fn remove_agent_everywhere(&self, agent_id: &str) -> Result<()>;
}

#[async_trait::async_trait]
impl WorkspaceStorage for VizierStorage {
    async fn create_workspace(
        &self,
        workspace: &Workspace,
        topic: &WorkspaceTopic,
        owner: &WorkspaceMember,
    ) -> Result<()> {
        self.0.create_workspace(workspace, topic, owner).await
    }

    async fn get_workspace(&self, id: &str) -> Result<Option<Workspace>> {
        self.0.get_workspace(id).await
    }

    async fn find_workspace_by_name(&self, name: &str) -> Result<Option<Workspace>> {
        self.0.find_workspace_by_name(name).await
    }

    async fn workspaces_of_person(&self, username: &str) -> Result<Vec<(Workspace, WorkspaceRole)>> {
        self.0.workspaces_of_person(username).await
    }

    async fn workspaces_of_agent(&self, agent_id: &str) -> Result<Vec<String>> {
        self.0.workspaces_of_agent(agent_id).await
    }

    async fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        self.0.list_workspaces().await
    }

    async fn rename_workspace(&self, id: &str, name: &str, now: i64) -> Result<()> {
        self.0.rename_workspace(id, name, now).await
    }

    async fn delete_workspace(&self, id: &str) -> Result<()> {
        self.0.delete_workspace(id).await
    }

    async fn list_topics(&self, workspace_id: &str) -> Result<Vec<WorkspaceTopic>> {
        self.0.list_topics(workspace_id).await
    }

    async fn get_topic(&self, workspace_id: &str, topic_id: &str) -> Result<Option<WorkspaceTopic>> {
        self.0.get_topic(workspace_id, topic_id).await
    }

    async fn find_topic_by_name(&self, workspace_id: &str, name: &str) -> Result<Option<WorkspaceTopic>> {
        self.0.find_topic_by_name(workspace_id, name).await
    }

    async fn create_topic(&self, topic: &WorkspaceTopic) -> Result<()> {
        self.0.create_topic(topic).await
    }

    async fn rename_topic(&self, workspace_id: &str, topic_id: &str, name: &str) -> Result<()> {
        self.0.rename_topic(workspace_id, topic_id, name).await
    }

    async fn delete_topic(&self, workspace_id: &str, topic_id: &str) -> Result<()> {
        self.0.delete_topic(workspace_id, topic_id).await
    }

    async fn list_members(&self, workspace_id: &str) -> Result<Vec<WorkspaceMember>> {
        self.0.list_members(workspace_id).await
    }

    async fn get_member(&self, workspace_id: &str, handle: &str) -> Result<Option<WorkspaceMember>> {
        self.0.get_member(workspace_id, handle).await
    }

    async fn add_member(&self, member: &WorkspaceMember) -> Result<()> {
        self.0.add_member(member).await
    }

    async fn set_member_role(&self, workspace_id: &str, handle: &str, role: WorkspaceRole) -> Result<()> {
        self.0.set_member_role(workspace_id, handle, role).await
    }

    async fn remove_member(&self, workspace_id: &str, kind: MemberKind, handle: &str) -> Result<()> {
        self.0.remove_member(workspace_id, kind, handle).await
    }

    async fn remove_agent_everywhere(&self, agent_id: &str) -> Result<()> {
        self.0.remove_agent_everywhere(agent_id).await
    }
}
