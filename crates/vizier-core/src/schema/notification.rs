//! The notification inbox (`specs/029-webui-notifications/data-model.md`): one row per notable
//! event, stored once and filtered by agent visibility at read. Read state is per person, and
//! unread is the absence of a read row.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    ChannelReplies,
    /// A WebUI conversation's reply. Addressed to the person whose conversation it is
    /// (`audience_user_id`), not to everyone who can see the agent.
    ChatReply,
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
            Self::ChannelReplies => "channel_replies",
            Self::ChatReply => "chat_reply",
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
            "channel_replies" => Some(Self::ChannelReplies),
            "chat_reply" => Some(Self::ChatReply),
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
    pub created_at: DateTime<Utc>,
}

/// One row as the API returns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct NotificationItem {
    /// Stable public id; cursors and read marks use it.
    pub id: i64,
    pub at: DateTime<Utc>,
    pub agent_id: String,
    pub category: NotificationCategory,
    pub tone: NotificationTone,
    /// The stored summary; the WebUI renders it as text.
    pub text: String,
    pub href: String,
    /// This person's read state.
    pub read: bool,
}

/// Counts over the whole visible, retained set, independent of the selected page/filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct NotificationCounts {
    pub all: u64,
    pub unread: u64,
    pub read: u64,
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

/// One agent's replies in one Discord or Telegram conversation within one closed hour, from
/// which the channel-reply projection makes one notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyBatch {
    pub agent_id: String,
    pub channel: String,
    /// Hours since the Unix epoch.
    pub hour: i64,
    pub replies: u64,
    /// The last reply in the bucket; becomes the notification's `created_at`.
    pub at: DateTime<Utc>,
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
            NotificationCategory::ChannelReplies,
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
