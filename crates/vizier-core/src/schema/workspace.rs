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
