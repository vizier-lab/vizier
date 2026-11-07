//! Workspaces, their topics and members (`specs/053-workspace-channel/data-model.md`). Storage
//! is unaware of permissions; a unique-constraint failure comes back as an error that
//! [`crate::storage::sql::is_constraint_violation`] recognises, and the service layer maps it.

use anyhow::Result;

use crate::{
    schema::{
        CatchUp, EntryMention, MemberKind, MentionState, NewEntry, SessionHistory, VizierSession,
        Workspace, WorkspaceEntry, WorkspaceMember, WorkspaceRole, WorkspaceTopic,
    },
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

    // --- the shared timeline ---------------------------------------------------------------

    /// Saves an entry and its mentions in one transaction. `mentions` carry their first state:
    /// `Pending` for an agent, `Resolved` for a person.
    async fn insert_entry(
        &self,
        entry: &NewEntry,
        mentions: &[(MemberKind, String, MentionState)],
    ) -> Result<WorkspaceEntry>;

    async fn get_entry(&self, seq: i64) -> Result<Option<WorkspaceEntry>>;

    /// The entry written from this `session_history` row.
    async fn entry_by_history_uid(&self, uid: &str) -> Result<Option<WorkspaceEntry>>;

    /// A page of a topic, oldest first. `before` is exclusive; none means the newest page.
    async fn list_entries(
        &self,
        topic_id: &str,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<WorkspaceEntry>>;

    /// The newest `seq` in a Workspace, or 0.
    async fn head_seq(&self, workspace_id: &str) -> Result<i64>;

    async fn mentions_of(&self, seqs: &[i64]) -> Result<Vec<EntryMention>>;

    /// Mentions of agents still waiting to be delivered, oldest first.
    async fn pending_mentions(&self) -> Result<Vec<EntryMention>>;

    async fn set_mention_state(&self, entry_seq: i64, target: &str, state: MentionState, now: i64)
        -> Result<()>;

    /// Stop all: marks the Workspace's pending mentions dropped and records the head, so a turn
    /// that finishes a moment later cannot restart the chain (research R7). Returns how many
    /// mentions were dropped and the head.
    async fn stop_all(&self, workspace_id: &str, now: i64) -> Result<(usize, i64)>;

    /// The entry that started an agent's current turn in a topic, or `None`. A tool that writes
    /// an entry while a turn runs carries it, so Stop all can tell a chain it already covered.
    async fn current_activation(&self, session: &VizierSession) -> Result<Option<i64>>;

    /// What the member agents did in a topic between two moments: the rows of their own
    /// sessions that are not requests (thoughts, calls, results, messages), oldest first. The
    /// timeline merges these with its entries by time (research R3); one query for the span of
    /// a page, using the `(channel, topic, timestamp)` index.
    async fn steps_between(
        &self,
        workspace_id: &str,
        topic_id: &str,
        from_ms: i64,
        to_ms: i64,
    ) -> Result<Vec<SessionHistory>>;

    // --- an agent's record -----------------------------------------------------------------

    /// Starts an agent reading every topic of a Workspace from now, with no backfill.
    async fn begin_reading_agent(&self, workspace_id: &str, agent_id: &str) -> Result<()>;

    /// Starts every member agent reading a new topic from now.
    async fn begin_reading_topic(&self, workspace_id: &str, topic_id: &str) -> Result<()>;

    async fn read_position(&self, topic_id: &str, agent_id: &str) -> Result<Option<i64>>;

    /// Copies the entries the agent has not read into its session as `SilentRead` requests, in
    /// the same transaction that moves its read position (research R2).
    ///
    /// Copies person and agent entries with `read < seq < copy_before` that the agent did not
    /// write itself, at most `cap` of the newest, then moves the position to `position`. An
    /// agent with no position row copies nothing.
    async fn catch_up(
        &self,
        session: &VizierSession,
        topic_id: &str,
        copy_before: i64,
        position: i64,
        cap: usize,
    ) -> Result<CatchUp>;
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

    async fn insert_entry(
        &self,
        entry: &NewEntry,
        mentions: &[(MemberKind, String, MentionState)],
    ) -> Result<WorkspaceEntry> {
        self.0.insert_entry(entry, mentions).await
    }

    async fn get_entry(&self, seq: i64) -> Result<Option<WorkspaceEntry>> {
        self.0.get_entry(seq).await
    }

    async fn entry_by_history_uid(&self, uid: &str) -> Result<Option<WorkspaceEntry>> {
        self.0.entry_by_history_uid(uid).await
    }

    async fn list_entries(
        &self,
        topic_id: &str,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<WorkspaceEntry>> {
        self.0.list_entries(topic_id, before, limit).await
    }

    async fn head_seq(&self, workspace_id: &str) -> Result<i64> {
        self.0.head_seq(workspace_id).await
    }

    async fn mentions_of(&self, seqs: &[i64]) -> Result<Vec<EntryMention>> {
        self.0.mentions_of(seqs).await
    }

    async fn pending_mentions(&self) -> Result<Vec<EntryMention>> {
        self.0.pending_mentions().await
    }

    async fn set_mention_state(
        &self,
        entry_seq: i64,
        target: &str,
        state: MentionState,
        now: i64,
    ) -> Result<()> {
        self.0.set_mention_state(entry_seq, target, state, now).await
    }

    async fn stop_all(&self, workspace_id: &str, now: i64) -> Result<(usize, i64)> {
        self.0.stop_all(workspace_id, now).await
    }

    async fn current_activation(&self, session: &VizierSession) -> Result<Option<i64>> {
        self.0.current_activation(session).await
    }

    async fn steps_between(
        &self,
        workspace_id: &str,
        topic_id: &str,
        from_ms: i64,
        to_ms: i64,
    ) -> Result<Vec<SessionHistory>> {
        self.0.steps_between(workspace_id, topic_id, from_ms, to_ms).await
    }

    async fn begin_reading_agent(&self, workspace_id: &str, agent_id: &str) -> Result<()> {
        self.0.begin_reading_agent(workspace_id, agent_id).await
    }

    async fn begin_reading_topic(&self, workspace_id: &str, topic_id: &str) -> Result<()> {
        self.0.begin_reading_topic(workspace_id, topic_id).await
    }

    async fn read_position(&self, topic_id: &str, agent_id: &str) -> Result<Option<i64>> {
        self.0.read_position(topic_id, agent_id).await
    }

    async fn catch_up(
        &self,
        session: &VizierSession,
        topic_id: &str,
        copy_before: i64,
        position: i64,
        cap: usize,
    ) -> Result<CatchUp> {
        self.0.catch_up(session, topic_id, copy_before, position, cap).await
    }
}
