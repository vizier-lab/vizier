use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::agents::tools::{ToolCategory, ToolContext, VizierTool};
use crate::error::VizierError;
use crate::notifications::Notifications;
use crate::schema::{
    AgentId, NEW_SESSION_TITLE, SessionHistoryContent, TOPIC_ID_LEN, VizierChannelId,
    VizierResponse, VizierResponseContent, VizierSession, VizierSessionDetail,
};
use crate::storage::{VizierStorage, history::HistoryStorage, session::SessionStorage};
use crate::transport::{InboxEvent, VizierTransport};

/// The channel every WebUI conversation lives on (`webui/app/api/endpoints/chat.ts`).
pub const WEBUI_CHANNEL: &str = "vizier-webui";

/// The longest title a new conversation is created with, so one call cannot store a paragraph.
const MAX_TITLE_CHARS: usize = 120;

/// One person's WebUI conversation: the session every WebUI tool addresses.
fn webui_session(agent_id: &AgentId, username: &str, topic: &str) -> VizierSession {
    VizierSession(
        agent_id.clone(),
        VizierChannelId::HTTP(username.to_string(), WEBUI_CHANNEL.to_string()),
        Some(topic.to_string()),
    )
}

pub struct SendWebuiMessage {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
    /// Where the saved reply is published, so an open tab shows it without a reload.
    pub transport: VizierTransport,
    /// The inbox row an open WebUI badges, and the toast it shows.
    pub notifications: Notifications,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SendWebuiMessageArgs {
    #[schemars(description = "username of the target webui user")]
    username: String,

    #[schemars(description = "id of the target topic")]
    topic_id: String,

    #[schemars(description = "content of the message")]
    content: String,
}

#[async_trait::async_trait]
impl VizierTool for SendWebuiMessage {
    type Input = SendWebuiMessageArgs;
    type Output = String;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Webui)
    }

    fn name() -> String {
        "webui_send_message".to_string()
    }

    fn description(&self) -> String {
        "post to a webui user's topic other than the current conversation. The message is stored \
         and pushed live to every tab that has the topic open, and it raises one inbox \
         notification for that person. Your reply to the current conversation is delivered \
         automatically; never use this for it. The topic must exist: use \
         `webui_create_conversation` first when it does not"
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let session = webui_session(&self.agent_id, &args.username, &args.topic_id);

        // A message to a topic with no detail row would show in the person's conversation list
        // untitled and be missing from their topic picker, so it is refused instead of stored.
        let known = self
            .storage
            .get_session_detail_by_topic(
                self.agent_id.clone(),
                session.1.clone(),
                session.2.clone(),
            )
            .await
            .map_err(|e| VizierError(e.to_string()))?;
        if known.is_none() {
            return Err(VizierError(format!(
                "unknown_topic: user {} has no WebUI conversation {}; create it with \
                 webui_create_conversation first",
                args.username, args.topic_id
            )));
        }

        let response = VizierResponse {
            timestamp: crate::time::now(),
            content: VizierResponseContent::Message {
                content: args.content,
                stats: None,
            },
            attachments: vec![],
            ..Default::default()
        };

        // The path a turn's reply takes: saved once, then published as
        // `SessionFrame::HistoryEntry` for every socket on the conversation.
        let Some(uid) = crate::agents::steps::record_step(
            &self.storage,
            &self.transport,
            &session,
            SessionHistoryContent::Response(response),
        )
        .await
        else {
            return Err(VizierError(
                "the message could not be saved to that conversation".into(),
            ));
        };

        // Best-effort, like every notification.
        self.notifications
            .chat_replied(&session, &uid, crate::time::now())
            .await;

        Ok(format!(
            "Message sent to user {} topic {}",
            args.username, args.topic_id
        ))
    }
}

pub struct CreateWebuiConversation {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
    pub transport: VizierTransport,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CreateWebuiConversationArgs {
    #[schemars(description = "username of the webui user the conversation belongs to")]
    username: String,

    #[schemars(
        description = "title the person will see; omit for a placeholder that a later turn titles"
    )]
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebuiConversationCreated {
    #[schemars(description = "id of the new topic; send to it with webui_send_message")]
    pub topic_id: String,

    #[schemars(description = "the title it was created with")]
    pub title: String,
}

/// A title of 1–[`MAX_TITLE_CHARS`] characters, or the placeholder.
fn conversation_title(title: Option<&str>) -> String {
    let title = title.unwrap_or("").trim();
    if title.is_empty() {
        return NEW_SESSION_TITLE.to_string();
    }
    title
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect::<String>()
        .trim_end()
        .to_string()
}

#[async_trait::async_trait]
impl VizierTool for CreateWebuiConversation {
    type Input = CreateWebuiConversationArgs;
    type Output = WebuiConversationCreated;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Webui)
    }

    fn name() -> String {
        "webui_create_conversation".to_string()
    }

    fn description(&self) -> String {
        "create an empty WebUI conversation for a person, so it is listed in their WebUI before \
         its first message and `webui_send_message` has somewhere to post. Give the title you \
         want them to see: no turn runs in a conversation created this way, so nothing else will \
         title it"
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        // A name nobody owns would otherwise make a conversation only this agent can find.
        let Some(user) =
            crate::storage::user::UserStorage::get_user(&*self.storage, &args.username)
                .await
                .map_err(|e| VizierError(e.to_string()))?
        else {
            return Err(VizierError(format!(
                "unknown_user: there is no person named {}",
                args.username
            )));
        };

        let title = conversation_title(args.title.as_deref());
        let topic = nanoid::nanoid!(TOPIC_ID_LEN);
        let detail = VizierSessionDetail {
            agent_id: self.agent_id.clone(),
            channel: VizierChannelId::HTTP(args.username.clone(), WEBUI_CHANNEL.to_string()),
            topic: Some(topic.clone()),
            title: title.clone(),
            is_thinking: false,
        };
        self.storage
            .save_session_detail(detail)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        // Their open tabs reload the conversation list, so the new row appears now rather than on
        // the sidebar's slow poll.
        self.transport.publish_inbox_event(InboxEvent::Stale {
            user: Some(user.user_id),
            agent_id: None,
        });

        Ok(WebuiConversationCreated {
            topic_id: topic,
            title,
        })
    }
}

pub struct ListWebuiTopics {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListWebuiTopicsArgs {
    #[schemars(description = "username of the target webui user")]
    username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebuiTopicEntry {
    #[schemars(description = "topic id")]
    pub topic_id: String,

    #[schemars(description = "topic title")]
    pub title: String,

    #[schemars(description = "whether the agent is currently thinking in this topic")]
    pub is_thinking: bool,
}

#[async_trait::async_trait]
impl VizierTool for ListWebuiTopics {
    type Input = ListWebuiTopicsArgs;
    type Output = Vec<WebuiTopicEntry>;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Webui)
    }

    fn name() -> String {
        "webui_list_topics".to_string()
    }

    fn description(&self) -> String {
        "list all webui topics for a given user".into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let channel = VizierChannelId::HTTP(args.username, WEBUI_CHANNEL.to_string());

        let sessions = self
            .storage
            .get_session_list(self.agent_id.clone(), Some(channel))
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        let topics: Vec<WebuiTopicEntry> = sessions
            .into_iter()
            .filter_map(|s| {
                let topic_id = s.topic?;
                Some(WebuiTopicEntry {
                    topic_id,
                    title: s.title,
                    is_thinking: s.is_thinking,
                })
            })
            .collect();

        Ok(topics)
    }
}

#[cfg(test)]
mod tests;
