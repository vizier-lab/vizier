//! The notification inbox (`specs/029-webui-notifications/data-model.md`): one row per notable
//! event, stored once and filtered by agent visibility at read. Read state is per person, and
//! unread is the absence of a read row.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use super::VizierSession;

/// What kind of event a notification records. Exposed for semantics and tests; never offered as
/// a filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum NotificationCategory {
    AgentStartFailure,
    AgentSetupReady,
    TaskRun,
    DreamCycle,
    BackgroundJob,
    Integration,
    /// A WebUI conversation's reply. Addressed to the person whose conversation it is
    /// (`audience_user_id`), not to everyone who can see the agent.
    ChatReply,
    /// A message in a Workspace (`specs/053-workspace-channel`), one row per message. Seen by
    /// every current member except its author.
    WorkspaceMessage,
}

impl NotificationCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AgentStartFailure => "agent_start_failure",
            Self::AgentSetupReady => "agent_setup_ready",
            Self::TaskRun => "task_run",
            Self::DreamCycle => "dream_cycle",
            Self::BackgroundJob => "background_job",
            Self::Integration => "integration",
            Self::ChatReply => "chat_reply",
            Self::WorkspaceMessage => "workspace_message",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "agent_start_failure" => Some(Self::AgentStartFailure),
            "agent_setup_ready" => Some(Self::AgentSetupReady),
            "task_run" => Some(Self::TaskRun),
            "dream_cycle" => Some(Self::DreamCycle),
            "background_job" => Some(Self::BackgroundJob),
            "integration" => Some(Self::Integration),
            "chat_reply" => Some(Self::ChatReply),
            "workspace_message" => Some(Self::WorkspaceMessage),
            _ => None,
        }
    }
}

/// How a row reads: ordinary, a failure, or a successful outcome.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum NotificationTone {
    #[default]
    Default,
    Failed,
    Success,
}

impl NotificationTone {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Failed => "failed",
            Self::Success => "success",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "default" => Some(Self::Default),
            "failed" => Some(Self::Failed),
            "success" => Some(Self::Success),
            _ => None,
        }
    }
}

/// Which rows a page returns. `all` is the default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum NotificationFilter {
    #[default]
    All,
    Unread,
    Read,
}

impl NotificationFilter {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Unread => "unread",
            Self::Read => "read",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "all" => Some(Self::All),
            "unread" => Some(Self::Unread),
            "read" => Some(Self::Read),
            _ => None,
        }
    }
}

/// The `agent_id` of a Workspace row starts with this and is followed by the Workspace id. The
/// service adds one such id for every Workspace a person is in to the agents they may see, so the
/// visibility clause every query already has covers Workspace rows too.
pub const WORKSPACE_SCOPE_PREFIX: &str = "workspace:";

/// The scope id of a Workspace's notifications.
pub fn workspace_scope(workspace_id: &str) -> String {
    format!("{WORKSPACE_SCOPE_PREFIX}{workspace_id}")
}

/// A row the `Notifications` service asks storage to insert. Not exposed by the API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationDraft {
    /// The exactly-once guard: one row per event, forever.
    pub event_key: String,
    pub agent_id: String,
    /// When set, only this person sees the row. Used by WebUI chat replies, whose
    /// conversation belongs to one person; every other category is visible to everyone who
    /// can see the agent (the default, `None`).
    pub audience_user_id: Option<String>,
    pub category: NotificationCategory,
    pub tone: NotificationTone,
    /// Plain text, built from stored facts. Never model output or a setting value.
    pub summary: String,
    pub href: String,
    pub created_at: DateTime<Local>,
    /// The conversation the row is about, when it has one. Its current title is filled on read,
    /// and the row goes away with the conversation.
    pub source_session: Option<VizierSession>,
    /// A stored title for a source with no session (a task, when its run ended).
    pub source_title: Option<String>,
    /// A short plain-text excerpt (`notifications::excerpt`). Never tool arguments, tool
    /// results or reasoning.
    pub excerpt: Option<String>,
    /// The Workspace a message was written in (spec 053).
    pub workspace_id: Option<String>,
    /// Who wrote it: a person's user id or an agent's id. Its author never sees the row.
    pub author: Option<String>,
}

/// A row the store has just inserted, as the live stream needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertedNotification {
    pub item: NotificationItem,
    /// The row's `audience_user_id`: `None` means everyone who can view the agent.
    pub audience: Option<String>,
    /// The row's author, so a live frame is not sent to them.
    pub author: Option<String>,
}

/// One row as the API returns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct NotificationItem {
    /// Stable public id; cursors and read marks use it.
    pub id: i64,
    pub at: DateTime<Local>,
    pub agent_id: String,
    pub category: NotificationCategory,
    pub tone: NotificationTone,
    /// The stored summary; the WebUI renders it as text.
    pub text: String,
    pub href: String,
    /// This person's read state.
    pub read: bool,
    /// The conversation's current title or the task's title. Absent when the source has none,
    /// or when the viewer may not see it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A short plain-text excerpt of what the source said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    /// Used by the service to fill and gate `title`; never sent.
    #[serde(skip)]
    pub source_session: Option<VizierSession>,
}

/// Counts over the whole visible, retained set, independent of the selected page/filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct NotificationCounts {
    pub all: u64,
    pub unread: u64,
    pub read: u64,
}

/// How many replies one person has not read in one of their own WebUI conversations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ChatUnread {
    pub agent_id: String,
    /// The conversation's in-app path, as its notifications link to it.
    pub href: String,
    pub unread: u64,
}

/// One page of the inbox plus the counts for its filters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct NotificationPage {
    pub items: Vec<NotificationItem>,
    pub counts: NotificationCounts,
    /// Pass as `before` for the next page; absent on the last one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_round_trip_through_their_strings() {
        for category in [
            NotificationCategory::AgentStartFailure,
            NotificationCategory::AgentSetupReady,
            NotificationCategory::TaskRun,
            NotificationCategory::DreamCycle,
            NotificationCategory::BackgroundJob,
            NotificationCategory::Integration,
            NotificationCategory::ChatReply,
        ] {
            assert_eq!(
                NotificationCategory::parse(category.as_str()),
                Some(category),
                "{}",
                category.as_str()
            );
        }
        assert_eq!(NotificationCategory::parse("unknown"), None);
    }

    #[test]
    fn tones_and_filters_round_trip() {
        for tone in [
            NotificationTone::Default,
            NotificationTone::Failed,
            NotificationTone::Success,
        ] {
            assert_eq!(NotificationTone::parse(tone.as_str()), Some(tone));
        }
        for filter in [
            NotificationFilter::All,
            NotificationFilter::Unread,
            NotificationFilter::Read,
        ] {
            assert_eq!(NotificationFilter::parse(filter.as_str()), Some(filter));
        }
        assert_eq!(NotificationFilter::default(), NotificationFilter::All);
        assert_eq!(NotificationTone::default(), NotificationTone::Default);
    }
}
