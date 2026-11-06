//! Workspace values (`specs/053-workspace-channel/data-model.md`). Storage is unaware of
//! permissions; `workspaces::access` and `service::workspaces` own those.

use serde::{Deserialize, Serialize};

/// A shared space with people, agents and topics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    /// A UUID that never changes, so a rename never changes a session key.
    pub id: String,
    pub name: String,
    pub created_by: String,
    /// The head `seq` when Stop all last ran (research R7).
    pub stopped_at_seq: i64,
    /// Unix milliseconds.
    pub created_at: i64,
    pub updated_at: i64,
}

/// A conversation inside a Workspace. Its id is the session's topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceTopic {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    /// A username, or `agent:<id>`.
    pub created_by: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberKind {
    Person,
    Agent,
}

impl MemberKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Agent => "agent",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "person" => Some(Self::Person),
            "agent" => Some(Self::Agent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRole {
    Owner,
    Member,
}

impl WorkspaceRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Member => "member",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "owner" => Some(Self::Owner),
            "member" => Some(Self::Member),
            _ => None,
        }
    }
}

/// A person or an agent in a Workspace. `handle` is a username or an agent id, and is unique in
/// the Workspace whatever the kind, so `@handle` is never ambiguous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMember {
    pub workspace_id: String,
    pub kind: MemberKind,
    pub handle: String,
    /// Agents are always `Member`.
    pub role: WorkspaceRole,
    pub added_by: String,
    pub added_at: i64,
}

/// What a timeline entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// A person's message.
    Person,
    /// An agent's mid-turn or final message.
    Agent,
    /// A line the system adds: a command someone ran. Never copied into an agent's record.
    Note,
}

impl EntryKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Agent => "agent",
            Self::Note => "note",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "person" => Some(Self::Person),
            "agent" => Some(Self::Agent),
            "note" => Some(Self::Note),
            _ => None,
        }
    }
}

/// A file shared with an entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryFile {
    pub file_id: String,
    pub name: String,
    pub mime_type: String,
    pub size: u64,
}

/// One shown message in a topic, in the one shared order (`seq`). Holds no tool calls or
/// thoughts: those stay in the author's own record (research R3).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceEntry {
    pub seq: i64,
    pub workspace_id: String,
    pub topic_id: String,
    pub kind: EntryKind,
    pub author_kind: MemberKind,
    /// A username or an agent id.
    pub author: String,
    /// The name as it was when written, so history survives a deleted agent.
    pub author_name: String,
    pub body: String,
    pub files: Vec<EntryFile>,
    /// The `session_history` row it was written from. `None` for a person's message and for one
    /// that `mention_agent` or `send_message_to_workspace` wrote.
    pub history_uid: Option<String>,
    /// The slug of the session that wrote it, so `catch_up` skips an agent's own text.
    pub source_session: Option<String>,
    /// Set when sent from outside: `{kind, label}`.
    pub origin: Option<serde_json::Value>,
    /// A turn's final reply. Its stats and reactions live on the history row.
    pub is_final: bool,
    /// The entry that started the turn this one belongs to. Used by the stop guard.
    pub activated_by_seq: Option<i64>,
    /// Unix milliseconds.
    pub at: i64,
}

/// An entry before it has a `seq`.
#[derive(Debug, Clone, PartialEq)]
pub struct NewEntry {
    pub workspace_id: String,
    pub topic_id: String,
    pub kind: EntryKind,
    pub author_kind: MemberKind,
    pub author: String,
    pub author_name: String,
    pub body: String,
    pub files: Vec<EntryFile>,
    pub history_uid: Option<String>,
    pub source_session: Option<String>,
    pub origin: Option<serde_json::Value>,
    pub is_final: bool,
    pub activated_by_seq: Option<i64>,
    pub at: i64,
}

/// Where a mention of an agent is. A person's mention is `Resolved` and only drives a highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MentionState {
    /// Saved, and not yet delivered. Delivered again at startup.
    Pending,
    /// The agent was sent a request.
    Woken,
    /// The agent could not be reached, so it was not woken.
    Unavailable,
    /// Dropped by Stop all.
    Dropped,
    /// A person mention.
    Resolved,
}

impl MentionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Woken => "woken",
            Self::Unavailable => "unavailable",
            Self::Dropped => "dropped",
            Self::Resolved => "resolved",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "pending" => Some(Self::Pending),
            "woken" => Some(Self::Woken),
            "unavailable" => Some(Self::Unavailable),
            "dropped" => Some(Self::Dropped),
            "resolved" => Some(Self::Resolved),
            _ => None,
        }
    }
}

/// One member named by an entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryMention {
    pub entry_seq: i64,
    pub target_kind: MemberKind,
    pub target: String,
    pub state: MentionState,
}

/// What a catch-up copied into an agent's record.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CatchUp {
    /// Entries copied.
    pub copied: usize,
    /// Older unread entries left out because only the latest are copied.
    pub skipped: usize,
}
