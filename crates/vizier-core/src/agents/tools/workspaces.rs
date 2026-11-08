//! The `workspaces` tools (`specs/053-workspace-channel/contracts/agent-tools.md`):
//! `mention_agent` (inside a Workspace), `send_message_to_workspace`, `list_workspaces` and
//! `read_workspace_messages` (from anywhere but a dream or a subagent).
//!
//! Every outcome but a storage failure is a `status` result a script can handle, never an
//! exception. A message a tool posts goes through the same path as one a person types, so a
//! mention wakes an agent the same way however it was written.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    schema::{
        EntryKind, MemberKind, MentionState, NewEntry, VizierChannelId, Workspace, WorkspaceTopic,
    },
    storage::{agent::AgentStorage, workspace::WorkspaceStorage},
    workspaces::{self, Hub, PostError, mentions, names},
};

fn storage_error(err: impl std::fmt::Display) -> VizierError {
    VizierError(format!("workspace storage: {err}"))
}

fn now_ms() -> i64 {
    crate::time::now().timestamp_millis()
}

/// Whether a session may use the tools that reach a Workspace from anywhere: everything but a
/// dream cycle and a subagent piece, which are unattended and have no place in a conversation.
fn reaches_workspaces(session: &crate::schema::VizierSession) -> bool {
    !matches!(session.1, VizierChannelId::Dream(..) | VizierChannelId::Subagent)
}

// --- composing the message `mention_agent` posts -------------------------------------------------

/// `agents` as handles: no `@`, no blanks, none twice.
pub fn handles_of(agents: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for agent in agents {
        let handle = agent.trim().trim_start_matches('@').to_string();
        if !handle.is_empty() && !out.iter().any(|h| h.eq_ignore_ascii_case(&handle)) {
            out.push(handle);
        }
    }
    out
}

/// The text posted: the message with `@a @b` in front, unless it already names them all. A
/// message that starts with a skill gets the mentions after it (`/status-note outage` becomes
/// `/status-note @a outage`), because a command is `/<command> @<agent-id> [input]`.
pub fn compose(message: &str, handles: &[String]) -> String {
    let names: Vec<&str> = handles.iter().map(String::as_str).collect();
    let named: Vec<String> = mentions::parse(message, &names).into_iter().map(|m| m.handle).collect();
    if handles.iter().all(|h| named.iter().any(|n| n.eq_ignore_ascii_case(h))) {
        return message.trim().to_string();
    }
    let tags = handles.iter().map(|h| format!("@{h}")).collect::<Vec<_>>().join(" ");
    let text = message.trim();
    if let Some(rest) = text.strip_prefix('/') {
        // `/skill name input` has two words before the input, `/name input` one.
        let mut words = rest.splitn(3, char::is_whitespace);
        let first = words.next().unwrap_or_default();
        let (head, input) = if first.eq_ignore_ascii_case("skill") {
            let name = words.next().unwrap_or_default();
            (format!("/{first} {name}"), words.next().unwrap_or_default())
        } else {
            (format!("/{first}"), rest[first.len()..].trim_start())
        };
        return if input.is_empty() { format!("{head} {tags}") } else { format!("{head} {tags} {input}") };
    }
    format!("{tags} {text}")
}

// --- shared lookups -------------------------------------------------------------------------------

/// The Workspace an agent addresses by name, when it is a member: the same `None` for one that
/// does not exist, so a name says nothing about what the agent may not see.
async fn visible_workspace(hub: &Hub, agent_id: &str, name: &str) -> Result<Option<Workspace>, VizierError> {
    let Some(workspace) = hub.storage.find_workspace_by_name(&names::normalize(name)).await.map_err(storage_error)? else {
        return Ok(None);
    };
    let member = hub.storage.get_member(&workspace.id, agent_id).await.map_err(storage_error)?;
    Ok(member.filter(|m| m.kind == MemberKind::Agent).map(|_| workspace))
}

/// What a posted entry came to: who was woken, and who could not be.
async fn outcome(hub: &Hub, seq: i64) -> Result<(Vec<String>, Vec<String>), VizierError> {
    let mentions = hub.storage.mentions_of(&[seq]).await.map_err(storage_error)?;
    let pick = |state| {
        mentions
            .iter()
            .filter(|m| m.target_kind == MemberKind::Agent && m.state == state)
            .map(|m| m.target.clone())
            .collect::<Vec<_>>()
    };
    Ok((pick(MentionState::Woken), pick(MentionState::Unavailable)))
}

async fn author_name(hub: &Hub, agent_id: &str) -> String {
    match hub.storage.get_agent(agent_id).await {
        Ok(Some(config)) => config.name,
        _ => agent_id.to_string(),
    }
}

/// Where a message sent from outside came from (FR-032). `None` from the same topic.
async fn origin_of(hub: &Hub, session: &crate::schema::VizierSession, workspace: &Workspace, topic: &WorkspaceTopic) -> Option<serde_json::Value> {
    let (kind, label) = match &session.1 {
        VizierChannelId::Task(slug, _) => ("task", slug.clone()),
        VizierChannelId::HTTP(..) | VizierChannelId::Project(..) => ("chat", "a direct chat".to_string()),
        VizierChannelId::DiscordChanel(_) => ("discord", "Discord".to_string()),
        VizierChannelId::TelegramChannel(_) => ("telegram", "Telegram".to_string()),
        VizierChannelId::Workspace(id) => {
            if *id == workspace.id && session.2.as_deref() == Some(topic.id.as_str()) {
                return None;
            }
            let name = hub.storage.get_workspace(id).await.ok().flatten().map_or_else(|| id.clone(), |w| w.name);
            ("workspace", name)
        }
        VizierChannelId::InterAgent(_) | VizierChannelId::System | VizierChannelId::Subagent | VizierChannelId::Dream(..) => return None,
    };
    Some(serde_json::json!({ "kind": kind, "label": label }))
}

async fn write_agent_entry(
    hub: &Hub,
    ctx: &ToolContext,
    workspace: &Workspace,
    topic: &WorkspaceTopic,
    body: String,
    origin: Option<serde_json::Value>,
) -> Result<Result<(i64, Vec<String>, Vec<String>), String>, VizierError> {
    let agent_id = ctx.session.0.clone();
    let draft = NewEntry {
        workspace_id: workspace.id.clone(),
        topic_id: topic.id.clone(),
        kind: EntryKind::Agent,
        author_kind: MemberKind::Agent,
        author: agent_id.clone(),
        author_name: author_name(hub, &agent_id).await,
        body,
        files: vec![],
        history_uid: None,
        // The session the call was made in, so a message from the topic itself is not read back
        // by its author, and one sent from elsewhere is (research R6).
        source_session: Some(ctx.session.to_slug()),
        origin,
        is_final: false,
        // A message an agent posts here is a message: everyone sees it, unlike its prose between
        // rounds, which stays in its own trail.
        shown: true,
        activated_by_seq: hub.storage.current_activation(&ctx.session).await.map_err(storage_error)?,
        at: now_ms(),
    };
    match workspaces::write_entry(hub, draft).await {
        Ok(entry) => {
            let (woken, unavailable) = outcome(hub, entry.seq).await?;
            Ok(Ok((entry.seq, woken, unavailable)))
        }
        Err(PostError::Empty) => Ok(Err("the message is empty".into())),
        Err(PostError::TooLong) => Ok(Err(format!("a message can be at most {} characters", workspaces::MAX_MESSAGE_CHARS))),
        Err(PostError::Refused(reason)) => Ok(Err(reason.to_string())),
        Err(PostError::Internal(e)) => Err(storage_error(e)),
    }
}

// --- mention_agent --------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct MentionAgentArgs {
    /// The ids of the agents to wake, with or without a leading `@`.
    pub agents: Vec<String>,
    /// What to say to them. `@` mentions are added in front, unless the text already names them.
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MentionResult {
    Posted { seq: i64, woken: Vec<String>, unavailable: Vec<String> },
    /// A named agent is not a member. Nothing was posted.
    NotAMember { agents: Vec<String>, members: Vec<String> },
    /// The caller named itself. Nothing was posted.
    SelfMention,
    Refused { message: String },
}

pub struct MentionAgent {
    pub hub: Hub,
}

#[async_trait::async_trait]
impl VizierTool for MentionAgent {
    type Input = MentionAgentArgs;
    type Output = MentionResult;

    fn name() -> String {
        "mention_agent".to_string()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Workspaces)
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        matches!(ctx.session.1, VizierChannelId::Workspace(_))
    }

    fn description(&self) -> String {
        "Wake other agents in this Workspace by mentioning them: it posts `@<agent-id> <message>` to the current topic, exactly as if you had written the mention yourself. They read what you say here, not your tool calls, and answer you only if they mention you back. A mention is a hand-off, not a name: it wakes that agent to act, and the agent cannot tell that apart from a passing remark. Mention an agent only when you want it to act, and refer to an agent by its bare name when you only mean to talk about it. Returns {status: \"posted\", seq, woken, unavailable}, {status: \"not_a_member\", members} listing who can be mentioned, or {status: \"self_mention\"}; nothing is posted unless the status is posted. A message that starts with a skill (`/status-note outage`) gets the mentions after it.".to_string()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let VizierChannelId::Workspace(workspace_id) = &ctx.session.1 else {
            return Ok(MentionResult::Refused { message: "mention_agent works only inside a Workspace".into() });
        };
        let Some(topic_id) = ctx.session.2.as_deref() else {
            return Ok(MentionResult::Refused { message: "this conversation has no topic".into() });
        };
        let hub = &self.hub;
        let me = ctx.session.0.as_str();
        let handles = handles_of(&args.agents);
        if handles.is_empty() {
            return Ok(MentionResult::Refused { message: "name at least one agent".into() });
        }
        if handles.iter().any(|h| h.eq_ignore_ascii_case(me)) {
            return Ok(MentionResult::SelfMention);
        }
        let members = hub.storage.list_members(workspace_id).await.map_err(storage_error)?;
        let agents: Vec<String> = members
            .iter()
            .filter(|m| m.kind == MemberKind::Agent && m.handle != me)
            .map(|m| m.handle.clone())
            .collect();
        let unknown: Vec<String> =
            handles.iter().filter(|h| !agents.iter().any(|a| a.eq_ignore_ascii_case(h))).cloned().collect();
        if !unknown.is_empty() {
            return Ok(MentionResult::NotAMember { agents: unknown, members: agents });
        }
        let (Some(workspace), Some(topic)) = (
            hub.storage.get_workspace(workspace_id).await.map_err(storage_error)?,
            hub.storage.get_topic(workspace_id, topic_id).await.map_err(storage_error)?,
        ) else {
            return Ok(MentionResult::Refused { message: "this Workspace or topic no longer exists".into() });
        };
        let body = compose(&args.message, &handles);
        Ok(match write_agent_entry(hub, ctx, &workspace, &topic, body, None).await? {
            Ok((seq, woken, unavailable)) => MentionResult::Posted { seq, woken, unavailable },
            Err(message) => MentionResult::Refused { message },
        })
    }
}

// --- send_message_to_workspace --------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SendToWorkspaceArgs {
    pub workspace_name: String,
    /// The topic's name. It is created when it does not exist.
    pub workspace_topic: String,
    /// The message. `@<agent-id>` in it wakes that member agent.
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SendResult {
    Posted { seq: i64, woken: Vec<String>, unavailable: Vec<String>, topic_created: bool },
    /// No such Workspace, **or you are not a member**: the same answer for both.
    NotFound,
    /// The topic name breaks the rule, so one cannot be created.
    InvalidName { rule: String },
    Refused { message: String },
}

pub struct SendMessageToWorkspace {
    pub hub: Hub,
}

#[async_trait::async_trait]
impl VizierTool for SendMessageToWorkspace {
    type Input = SendToWorkspaceArgs;
    type Output = SendResult;

    fn name() -> String {
        "send_message_to_workspace".to_string()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Workspaces)
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        reaches_workspaces(&ctx.session)
    }

    fn description(&self) -> String {
        "Post a message to a topic of a Workspace you are a member of, from any conversation, including a scheduled task. `@<agent-id>` in the message wakes that member agent, so a task can orchestrate a Workspace. A mention is a hand-off, not a name: the agent cannot tell it apart from a passing remark, so write it only when you want that agent to act, and refer to an agent by its bare name otherwise. The topic is created when it does not exist. The message shows where it came from. There is no return path: this conversation is not woken by the replies. Returns {status: \"posted\", seq, woken, unavailable, topic_created}, {status: \"not_found\"} (a missing Workspace and one you are not in answer the same), or {status: \"invalid_name\", rule}.".to_string()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let hub = &self.hub;
        let Some(workspace) = visible_workspace(hub, &ctx.session.0, &args.workspace_name).await? else {
            return Ok(SendResult::NotFound);
        };
        let key = names::normalize(&args.workspace_topic);
        let (topic, created) = match hub.storage.find_topic_by_name(&workspace.id, &key).await.map_err(storage_error)? {
            Some(topic) => (topic, false),
            None => {
                if names::validate(&key).is_err() {
                    return Ok(SendResult::InvalidName { rule: names::NameError::RULE.into() });
                }
                let topic = WorkspaceTopic {
                    id: uuid::Uuid::new_v4().to_string(),
                    workspace_id: workspace.id.clone(),
                    name: key,
                    created_by: format!("agent:{}", ctx.session.0),
                    created_at: now_ms(),
                };
                hub.storage.create_topic(&topic).await.map_err(storage_error)?;
                hub.storage.begin_reading_topic(&workspace.id, &topic.id).await.map_err(storage_error)?;
                (topic, true)
            }
        };
        let origin = origin_of(hub, &ctx.session, &workspace, &topic).await;
        Ok(match write_agent_entry(hub, ctx, &workspace, &topic, args.message, origin).await? {
            Ok((seq, woken, unavailable)) => SendResult::Posted { seq, woken, unavailable, topic_created: created },
            Err(message) => SendResult::Refused { message },
        })
    }
}

// --- list_workspaces ------------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListWorkspacesArgs {}

#[derive(Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
pub struct WorkspaceMembers {
    pub agents: Vec<String>,
    pub people: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
pub struct WorkspaceListing {
    pub name: String,
    pub topics: Vec<String>,
    pub members: WorkspaceMembers,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
pub struct ListWorkspacesResult {
    pub workspaces: Vec<WorkspaceListing>,
}

pub struct ListWorkspaces {
    pub hub: Hub,
}

#[async_trait::async_trait]
impl VizierTool for ListWorkspaces {
    type Input = ListWorkspacesArgs;
    type Output = ListWorkspacesResult;

    fn name() -> String {
        "list_workspaces".to_string()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Workspaces)
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        reaches_workspaces(&ctx.session)
    }

    fn description(&self) -> String {
        "List the Workspaces you are a member of, with their topics and who is in them. Returns {workspaces: [{name, topics, members: {agents, people}}]}; empty when you are in none.".to_string()
    }

    async fn call(&self, _args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let hub = &self.hub;
        let mut listing = Vec::new();
        for id in hub.storage.workspaces_of_agent(&ctx.session.0).await.map_err(storage_error)? {
            let Some(workspace) = hub.storage.get_workspace(&id).await.map_err(storage_error)? else { continue };
            let topics = hub.storage.list_topics(&id).await.map_err(storage_error)?.into_iter().map(|t| t.name).collect();
            let members = hub.storage.list_members(&id).await.map_err(storage_error)?;
            let by = |kind| members.iter().filter(|m| m.kind == kind).map(|m| m.handle.clone()).collect::<Vec<_>>();
            listing.push(WorkspaceListing {
                name: workspace.name,
                topics,
                members: WorkspaceMembers { agents: by(MemberKind::Agent), people: by(MemberKind::Person) },
            });
        }
        listing.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(ListWorkspacesResult { workspaces: listing })
    }
}

// --- read_workspace_messages ----------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadMessagesArgs {
    pub workspace_name: String,
    pub workspace_topic: String,
    /// How many to return: 50 by default, at most 200.
    #[serde(default)]
    pub limit: Option<usize>,
    /// Only messages older than this `seq`, to page back.
    #[serde(default)]
    pub before_seq: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
pub struct MessageView {
    pub seq: i64,
    pub author: String,
    pub body: String,
    /// Unix milliseconds.
    pub at: i64,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ReadResult {
    Found { messages: Vec<MessageView>, next_before_seq: Option<i64> },
    NotFound,
    /// The Workspace exists but has no such topic. Reading never creates one.
    TopicNotFound,
}

pub struct ReadWorkspaceMessages {
    pub hub: Hub,
}

#[async_trait::async_trait]
impl VizierTool for ReadWorkspaceMessages {
    type Input = ReadMessagesArgs;
    type Output = ReadResult;

    fn name() -> String {
        "read_workspace_messages".to_string()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Workspaces)
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        reaches_workspaces(&ctx.session)
    }

    fn description(&self) -> String {
        "Read the messages of a topic in a Workspace you are a member of, oldest first, text only (no tool calls). Reading does not mark anything as read for you. Returns {status: \"found\", messages: [{seq, author, body, at}], next_before_seq}, {status: \"not_found\"} (a missing Workspace and one you are not in answer the same), or {status: \"topic_not_found\"}. Pass next_before_seq as before_seq for earlier messages.".to_string()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let hub = &self.hub;
        let Some(workspace) = visible_workspace(hub, &ctx.session.0, &args.workspace_name).await? else {
            return Ok(ReadResult::NotFound);
        };
        let Some(topic) = hub.storage.find_topic_by_name(&workspace.id, &names::normalize(&args.workspace_topic)).await.map_err(storage_error)? else {
            return Ok(ReadResult::TopicNotFound);
        };
        let limit = args.limit.unwrap_or(50).clamp(1, 200);
        let entries = hub.storage.list_entries(&topic.id, args.before_seq, limit).await.map_err(storage_error)?;
        let next_before_seq = (entries.len() == limit).then(|| entries[0].seq);
        let messages = entries
            .into_iter()
            .filter(|e| e.kind != EntryKind::Note)
            .map(|e| MessageView { seq: e.seq, author: e.author, body: e.body, at: e.at })
            .collect();
        Ok(ReadResult::Found { messages, next_before_seq })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::schema::{WorkspaceMember, WorkspaceRole};
    use crate::storage::{VizierStorage, sqlite::SqlStorage};
    use crate::transport::VizierTransport;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|i| i.to_string()).collect()
    }

    #[test]
    fn handles_lose_their_at_and_repeats() {
        assert_eq!(handles_of(&s(&["@alpha", "beta", "ALPHA", " ", "@"])), s(&["alpha", "beta"]));
    }

    #[test]
    fn the_mentions_go_in_front_unless_the_text_already_has_them() {
        let both = s(&["alpha", "beta"]);
        assert_eq!(compose("have we seen this?", &both), "@alpha @beta have we seen this?");
        assert_eq!(compose("@alpha @beta have we seen this?", &both), "@alpha @beta have we seen this?");
        assert_eq!(compose("hey @alpha, and @beta too", &both), "hey @alpha, and @beta too");
        // One of two named: the missing one is not skipped.
        assert_eq!(compose("@alpha look", &both), "@alpha @beta @alpha look");
    }

    #[test]
    fn a_skill_keeps_its_place_and_the_mentions_follow_it() {
        let one = s(&["alpha"]);
        assert_eq!(compose("/status-note outage", &one), "/status-note @alpha outage");
        assert_eq!(compose("/status-note", &one), "/status-note @alpha");
        assert_eq!(compose("/skill status-note outage", &one), "/skill status-note @alpha outage");
        assert_eq!(compose("/greet @alpha Rafi", &one), "/greet @alpha Rafi");
    }

    fn hub() -> Hub {
        let store = crate::tests_support::test_store(|| {
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
            crate::storage::sqlite::init_history_schema(&conn).unwrap();
            crate::storage::sqlite::init_reaction_schema(&conn).unwrap();
            crate::storage::sqlite::workspace::init_workspace_schema(&conn).unwrap();
            conn.execute_batch(
                "CREATE TABLE agent_config (agent_id TEXT PRIMARY KEY, data TEXT NOT NULL); \
                 CREATE TABLE artifact_reference (artifact_id TEXT, agent_id TEXT, kind TEXT, \
                 target TEXT, session_slug TEXT, history_uid TEXT, channel TEXT, topic TEXT, \
                 bundle TEXT, path TEXT, label TEXT, created_at INTEGER, \
                 UNIQUE (artifact_id, kind, target));",
            )
            .unwrap();
            crate::storage::sql::SqlStore::sqlite(conn)
        });
        let dir = tempfile::tempdir().unwrap();
        let storage = SqlStorage::new(
            store,
            Arc::new(crate::storage::document::BlobDocumentStore::local(dir.keep())),
        );
        Hub::new(VizierStorage::new(storage), VizierTransport::new())
    }

    fn ctx(session: crate::schema::VizierSession) -> ToolContext {
        ToolContext {
            session,
            pending_attachments: Default::default(),
            hooks: None,
            background_depth: 0,
            loaded_images: Default::default(),
            recalled: Default::default(),
            deadline: None,
            client: None,
            questions: crate::questions::QuestionsHandle::new(Default::default()),
            interactive: false,
            secret_redactor: Default::default(),
            dream_sources: vec![],
            project: None,
        }
    }

    /// `ops` with agents alpha, beta, gamma and the topic `general`; returns its ids.
    async fn room(hub: &Hub) -> (String, String) {
        let now = 1;
        let ws = Workspace { id: "w1".into(), name: "ops".into(), created_by: "rafi".into(), stopped_at_seq: 0, created_at: now, updated_at: now };
        let topic = WorkspaceTopic { id: "t1".into(), workspace_id: "w1".into(), name: "general".into(), created_by: "rafi".into(), created_at: now };
        let owner = WorkspaceMember { workspace_id: "w1".into(), kind: MemberKind::Person, handle: "rafi".into(), role: WorkspaceRole::Owner, added_by: "rafi".into(), added_at: now };
        hub.storage.create_workspace(&ws, &topic, &owner).await.unwrap();
        for agent in ["alpha", "beta", "gamma"] {
            hub.storage
                .add_member(&WorkspaceMember { workspace_id: "w1".into(), kind: MemberKind::Agent, handle: agent.into(), role: WorkspaceRole::Member, added_by: "rafi".into(), added_at: now })
                .await
                .unwrap();
        }
        ("w1".into(), "t1".into())
    }

    fn in_room(agent: &str) -> ToolContext {
        ctx(crate::workspaces::session_of(agent, "w1", "t1"))
    }

    #[tokio::test]
    async fn mention_agent_is_offered_only_inside_a_workspace() {
        let tool = MentionAgent { hub: hub() };
        assert!(tool.available(&in_room("alpha")));
        let dm = ctx(crate::schema::VizierSession("alpha".into(), VizierChannelId::HTTP("rafi".into(), "x".into()), Some("t".into())));
        assert!(!tool.available(&dm));
    }

    #[tokio::test]
    async fn naming_a_non_member_or_yourself_posts_nothing() {
        let hub = hub();
        room(&hub).await;
        let tool = MentionAgent { hub: hub.clone() };
        let out = tool.call(MentionAgentArgs { agents: s(&["alpha", "stranger"]), message: "hi".into() }, &in_room("beta")).await.unwrap();
        assert_eq!(out, MentionResult::NotAMember { agents: s(&["stranger"]), members: s(&["alpha", "gamma"]) });
        let me = tool.call(MentionAgentArgs { agents: s(&["@beta"]), message: "hi".into() }, &in_room("beta")).await.unwrap();
        assert_eq!(me, MentionResult::SelfMention);
        assert!(hub.storage.list_entries("t1", None, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_mention_posts_an_entry_with_no_history_row_and_says_who_woke() {
        let hub = hub();
        room(&hub).await;
        // Gamma is up (it has a request queue); alpha, named too, is down.
        let _gamma = hub.transport.register_agent("gamma".into()).await;
        let tool = MentionAgent { hub: hub.clone() };
        let out = tool.call(MentionAgentArgs { agents: s(&["gamma", "alpha"]), message: "have we seen this?".into() }, &in_room("beta")).await.unwrap();
        let MentionResult::Posted { seq, woken, unavailable } = out else { panic!("expected posted, got {out:?}") };
        assert_eq!((woken, unavailable), (s(&["gamma"]), s(&["alpha"])));

        let entry = hub.storage.get_entry(seq).await.unwrap().unwrap();
        assert_eq!((entry.kind, entry.author.as_str(), entry.history_uid.as_deref()), (EntryKind::Agent, "beta", None));
        assert_eq!(entry.body, "@gamma @alpha have we seen this?");
        // Written while the script runs: nothing was added to the author's record.
        let record = crate::storage::history::HistoryStorage::list_session_history(&hub.storage, crate::workspaces::session_of("beta", "w1", "t1"), None, None, None).await.unwrap();
        assert!(record.is_empty());
        // The author's own session wrote it, so it is not read back by its author.
        assert_eq!(entry.source_session, Some(crate::workspaces::session_of("beta", "w1", "t1").to_slug()));
    }

    #[tokio::test]
    async fn every_named_agent_down_is_reported_the_same_way() {
        let hub = hub();
        room(&hub).await;
        let tool = MentionAgent { hub: hub.clone() };
        let out = tool.call(MentionAgentArgs { agents: s(&["alpha"]), message: "hi".into() }, &in_room("beta")).await.unwrap();
        assert!(matches!(out, MentionResult::Posted { ref woken, ref unavailable, .. } if woken.is_empty() && unavailable == &s(&["alpha"])), "{out:?}");
    }

    #[tokio::test]
    async fn from_outside_a_workspace_the_name_hides_what_the_agent_may_not_see() {
        let hub = hub();
        room(&hub).await;
        let task = ctx(crate::schema::VizierSession("alpha".into(), VizierChannelId::Task("daily".into(), crate::time::now()), None));
        let outsider = ctx(crate::schema::VizierSession("zed".into(), VizierChannelId::HTTP("rafi".into(), "x".into()), Some("t".into())));
        let send = SendMessageToWorkspace { hub: hub.clone() };
        let args = |ws: &str, topic: &str| SendToWorkspaceArgs { workspace_name: ws.into(), workspace_topic: topic.into(), message: "@beta tools".into() };

        // Not a member, and a name that does not exist, answer the same.
        assert_eq!(send.call(args("ops", "daily"), &outsider).await.unwrap(), SendResult::NotFound);
        assert_eq!(send.call(args("nope", "daily"), &task).await.unwrap(), SendResult::NotFound);
        assert_eq!(send.call(args("ops", "Not Ok!"), &task).await.unwrap(), SendResult::InvalidName { rule: names::NameError::RULE.into() });

        let out = send.call(args("OPS", "daily"), &task).await.unwrap();
        let SendResult::Posted { seq, topic_created, .. } = out else { panic!("{out:?}") };
        assert!(topic_created);
        let entry = hub.storage.get_entry(seq).await.unwrap().unwrap();
        assert_eq!(entry.origin, Some(serde_json::json!({ "kind": "task", "label": "daily" })));
        // A task's slug is not the topic's session, so the sender reads it back later.
        assert_ne!(entry.source_session, Some(crate::workspaces::session_of("alpha", "w1", &entry.topic_id).to_slug()));
        // The new topic is read from now by every member agent.
        assert!(hub.storage.read_position(&entry.topic_id, "beta").await.unwrap().is_some());
        // Sent again, the topic is found, not made.
        let again = send.call(args("ops", "daily"), &task).await.unwrap();
        assert!(matches!(again, SendResult::Posted { topic_created: false, .. }));
    }

    #[tokio::test]
    async fn listing_and_reading_cover_only_the_agents_own_workspaces() {
        let hub = hub();
        room(&hub).await;
        let task = ctx(crate::schema::VizierSession("alpha".into(), VizierChannelId::Task("daily".into(), crate::time::now()), None));
        let list = ListWorkspaces { hub: hub.clone() }.call(ListWorkspacesArgs {}, &task).await.unwrap();
        assert_eq!(list.workspaces.len(), 1);
        assert_eq!(list.workspaces[0].members.agents, s(&["alpha", "beta", "gamma"]));
        assert_eq!(list.workspaces[0].members.people, s(&["rafi"]));
        let none = ListWorkspaces { hub: hub.clone() }.call(ListWorkspacesArgs {}, &ctx(crate::schema::VizierSession("zed".into(), VizierChannelId::System, None))).await.unwrap();
        assert!(none.workspaces.is_empty());

        let read = ReadWorkspaceMessages { hub: hub.clone() };
        let at = |topic: &str| ReadMessagesArgs { workspace_name: "ops".into(), workspace_topic: topic.into(), limit: Some(2), before_seq: None };
        assert_eq!(read.call(at("missing"), &task).await.unwrap(), ReadResult::TopicNotFound);
        let mention = MentionAgent { hub: hub.clone() };
        for text in ["one", "two", "three"] {
            mention.call(MentionAgentArgs { agents: s(&["gamma"]), message: text.into() }, &in_room("beta")).await.unwrap();
        }
        let ReadResult::Found { messages, next_before_seq } = read.call(at("general"), &task).await.unwrap() else { panic!() };
        assert_eq!(messages.iter().map(|m| m.body.as_str()).collect::<Vec<_>>(), ["@gamma two", "@gamma three"]);
        assert_eq!(next_before_seq, Some(messages[0].seq));
        // Reading did not move anything: no read position for the reader changed.
        assert!(hub.storage.read_position("t1", "alpha").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn dreams_and_subagents_have_none_of_the_reaching_tools() {
        let hub = hub();
        let session = |channel| ctx(crate::schema::VizierSession("alpha".into(), channel, None));
        let dream = session(VizierChannelId::Dream(Box::new(crate::workspaces::session_of("alpha", "w1", "t1")), crate::schema::DreamStage::Extraction));
        let sub = session(VizierChannelId::Subagent);
        for c in [&dream, &sub] {
            assert!(!SendMessageToWorkspace { hub: hub.clone() }.available(c));
            assert!(!ListWorkspaces { hub: hub.clone() }.available(c));
            assert!(!ReadWorkspaceMessages { hub: hub.clone() }.available(c));
        }
        assert!(SendMessageToWorkspace { hub }.available(&session(VizierChannelId::System)));
    }
}
