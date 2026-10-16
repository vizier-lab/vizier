use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


pub type AgentId = String;

pub type TopicId = String;

#[derive(
    Debug,
    Clone,
    Hash,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    utoipa::ToSchema,
)]
pub struct VizierSession(pub AgentId, pub VizierChannelId, pub Option<TopicId>);

impl VizierSession {
    /// A scheduled task run — not a dream cycle, not an interactive turn.
    ///
    /// The question the scheduled-run framing needs answered, with one home. The
    /// request content kind cannot answer it: the dream cycle sends
    /// `VizierRequestContent::Unattended` too, while carrying its own framing.
    pub fn is_scheduled_task(&self) -> bool {
        matches!(self.1, VizierChannelId::Task(..))
    }

    /// The Channel-breakdown key of the usage page (`specs/018-usage-accounting/` R9): one per
    /// Discord channel, Telegram chat and WebUI topic; every run of one task together; dreams and
    /// background work each as one place.
    pub fn usage_place(&self) -> String {
        match &self.1 {
            VizierChannelId::DiscordChanel(id) => format!("discord:{id}"),
            VizierChannelId::TelegramChannel(id) => format!("telegram:{id}"),
            // The whole slug: the same user talks through several HTTP channels (the WebUI, the
            // REST API's `channel_id`), and the label lookup needs the exact one.
            VizierChannelId::HTTP(..) => {
                format!("http:{}:{}", self.1.to_slug(), self.2.as_deref().unwrap_or(""))
            }
            VizierChannelId::Task(slug, _) => format!("task:{slug}"),
            VizierChannelId::Subagent | VizierChannelId::InterAgent(_) => "background".into(),
            VizierChannelId::Dream(..) => "dreams".into(),
            VizierChannelId::System => "system".into(),
        }
    }

    pub fn to_slug(&self) -> String {
        format!(
            "{}__{}__{}",
            self.0,
            self.1.to_slug(),
            self.2.clone().unwrap_or("DEFAULT".to_string())
        )
    }
}

#[derive(
    Debug,
    Clone,
    Hash,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    utoipa::ToSchema,
)]
pub enum VizierChannelId {
    DiscordChanel(u64),
    TelegramChannel(i64),
    HTTP(String, String),
    Task(String, DateTime<Utc>),
    InterAgent(Vec<String>),
    System,
    Subagent,
    Dream(Box<VizierSession>, DreamStage),
}

impl VizierChannelId {
    /// The chat platform a conversation is happening on, if it is a Discord or Telegram one.
    pub fn platform(&self) -> Option<crate::schema::ChannelPlatform> {
        match self {
            Self::DiscordChanel(_) => Some(crate::schema::ChannelPlatform::Discord),
            Self::TelegramChannel(_) => Some(crate::schema::ChannelPlatform::Telegram),
            _ => None,
        }
    }

    /// What a turn in this conversation is, for usage accounting (`specs/018-usage-accounting/`
    /// R4). Handovers and vision reads are classified by their purpose instead.
    pub fn usage_activity(&self) -> crate::schema::usage::UsageActivity {
        use crate::schema::usage::UsageActivity;
        match self {
            Self::DiscordChanel(_) | Self::TelegramChannel(_) | Self::HTTP(..) => UsageActivity::Chat,
            Self::Task(..) => UsageActivity::Scheduled,
            Self::Subagent | Self::InterAgent(_) => UsageActivity::Background,
            Self::Dream(..) => UsageActivity::Dreams,
            Self::System => UsageActivity::Other,
        }
    }

    pub fn to_slug(&self) -> String {
        match self {
            Self::DiscordChanel(id) => format!("discord__{}", id),
            Self::TelegramChannel(id) => format!("telegram__{}", id),
            Self::HTTP(user, id) => format!("http__{}__{}", user, id),
            Self::Task(id, datetime) => {
                // The full timestamp, so each firing of a task gets its own
                // conversation. This used to render `timestamp_subsec_nanos()` of
                // a second-truncated time, which is always `0` — so every run of a
                // task collided into `task__{id}__0`.
                format!("task__{}__{}", id, datetime.timestamp_millis())
            }
            Self::InterAgent(set) => {
                let participants = set.join("__");

                format!("inter_agent__[{participants}]")
            }
            Self::System => "SYSTEM".into(),
            Self::Dream(session, stage) => {
                let stage_str = match stage {
                    DreamStage::Extraction => "extraction",
                    DreamStage::Consolidation => "consolidation",
                };
                format!("DREAM__{}__{}", session.to_slug(), stage_str)
            }
            Self::Subagent => "SUBAGENT".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VizierSessionDetail {
    pub agent_id: AgentId,
    pub channel: VizierChannelId,
    pub topic: Option<TopicId>,
    pub title: String,
    #[serde(default)]
    pub is_thinking: bool,
}

#[derive(
    Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DreamStage {
    Extraction,
    Consolidation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DreamStatus {
    Idle,
    Extracting {
        started_at: DateTime<Utc>,
        cycle_id: String,
        total_sessions: usize,
        completed_sessions: usize,
    },
    Consolidating {
        started_at: DateTime<Utc>,
        cycle_id: String,
    },
}
#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn at(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(seconds, 0).single().unwrap()
    }

    fn task_session(slug: &str, seconds: i64) -> VizierSession {
        VizierSession(
            "agent-1".to_string(),
            VizierChannelId::Task(slug.to_string(), at(seconds)),
            None,
        )
    }

    /// **The highest-leverage line in the feature.** The slug used to render
    /// `timestamp_subsec_nanos()` of a time the scheduler had already truncated to the whole
    /// second — always `0` — so every run of a task collided into `task__{slug}__0`. A
    /// recurring task replayed every previous run as history on each fire, and there was
    /// nothing for a run list to list.
    #[test]
    fn to_slug_yields_a_distinct_key_per_timestamp() {
        let first = task_session("tick", 1_700_000_000).to_slug();
        let second = task_session("tick", 1_700_000_060).to_slug();
        let third = task_session("tick", 1_700_000_120).to_slug();

        assert_ne!(first, second);
        assert_ne!(second, third);
        assert_ne!(first, third);

        assert_eq!(first, "agent-1__task__tick__1700000000000__DEFAULT");
    }

    /// Two tasks firing at the same instant stay separate, so the timestamp is not the whole
    /// identity.
    #[test]
    fn two_tasks_firing_at_once_do_not_collide() {
        assert_ne!(
            task_session("tick", 1_700_000_000).to_slug(),
            task_session("tock", 1_700_000_000).to_slug()
        );
    }

    /// A legacy conversation under the old `__0` key stays addressable: the variant's shape
    /// is unchanged and only its rendering moved, so the epoch still renders `0` and nothing
    /// pre-existing becomes unreachable. Those surface as one run per task and are not split
    /// retroactively — inferring run boundaries from timestamps inside one merged
    /// conversation would be guesswork.
    #[test]
    fn a_legacy_zero_suffixed_key_is_still_reachable() {
        assert_eq!(
            task_session("tick", 0).to_slug(),
            "agent-1__task__tick__0__DEFAULT"
        );
    }

    /// The question the scheduled-run framing asks, and the one place it is answered.
    #[test]
    fn is_scheduled_task_is_true_only_for_a_task_channel() {
        assert!(task_session("tick", 1_700_000_000).is_scheduled_task());

        let dream = VizierSession(
            "agent-1".to_string(),
            VizierChannelId::Dream(
                Box::new(task_session("tick", 1_700_000_000)),
                DreamStage::Extraction,
            ),
            None,
        );
        assert!(
            !dream.is_scheduled_task(),
            "the dream cycle sends the same request content kind and must not be framed"
        );

        for channel in [
            VizierChannelId::HTTP("someone".to_string(), "webui".to_string()),
            VizierChannelId::System,
            VizierChannelId::Subagent,
            VizierChannelId::DiscordChanel(1),
            VizierChannelId::TelegramChannel(1),
            VizierChannelId::InterAgent(vec!["agent-2".to_string()]),
        ] {
            assert!(
                !VizierSession("agent-1".to_string(), channel.clone(), None).is_scheduled_task(),
                "{channel:?} is not a scheduled task"
            );
        }
    }

    #[test]
    fn every_channel_has_a_usage_activity_and_place() {
        use crate::schema::usage::UsageActivity as A;
        let s = |channel, topic: Option<&str>| {
            VizierSession("a".into(), channel, topic.map(str::to_string))
        };
        let cases = [
            (s(VizierChannelId::DiscordChanel(7), None), A::Chat, "discord:7"),
            (s(VizierChannelId::TelegramChannel(-5), None), A::Chat, "telegram:-5"),
            (
                s(VizierChannelId::HTTP("alice".into(), "webui".into()), Some("t1")),
                A::Chat,
                "http:http__alice__webui:t1",
            ),
            (task_session("brief", 1_700_000_000), A::Scheduled, "task:brief"),
            (s(VizierChannelId::Subagent, None), A::Background, "background"),
            (
                s(VizierChannelId::InterAgent(vec!["a".into(), "b".into()]), None),
                A::Background,
                "background",
            ),
            (
                s(
                    VizierChannelId::Dream(
                        Box::new(s(VizierChannelId::DiscordChanel(7), None)),
                        DreamStage::Extraction,
                    ),
                    None,
                ),
                A::Dreams,
                "dreams",
            ),
            (s(VizierChannelId::System, None), A::Other, "system"),
        ];
        for (session, activity, place) in cases {
            assert_eq!(session.1.usage_activity(), activity, "{session:?}");
            assert_eq!(session.usage_place(), place, "{session:?}");
        }
    }
}
