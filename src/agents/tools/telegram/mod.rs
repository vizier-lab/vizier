use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use teloxide::Bot;
use teloxide::prelude::*;
use teloxide::sugar::request::RequestReplyExt;

use crate::agents::tools::{ToolContext, VizierTool};
use crate::error::{VizierError, throw_vizier_error};
use crate::schema::{AgentId, TopicId, VizierChannelId, VizierResponse, VizierResponseContent, VizierSession};
use crate::storage::{VizierStorage, history::HistoryStorage, state::StateStorage};

#[derive(Debug, Deserialize, Serialize)]
struct ChannelState {
    active_topic: Option<TopicId>,
}

pub fn new_telegram_tools(
    bot_token: String,
    agent_id: AgentId,
    storage: Arc<VizierStorage>,
) -> (
    SendTelegramMessage,
    ReactTelegramMessage,
    GetTelegramMessage,
) {
    let bot = Bot::new(bot_token);

    (
        SendTelegramMessage { bot: bot.clone(), agent_id: agent_id.clone(), storage: storage.clone() },
        ReactTelegramMessage { bot: bot.clone() },
        GetTelegramMessage { bot: bot.clone() },
    )
}

pub struct SendTelegramMessage {
    bot: Bot,
    agent_id: AgentId,
    storage: Arc<VizierStorage>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SendTelegramMessageArgs {
    #[schemars(description = "id of target telegram chat")]
    chat_id: i64,

    #[schemars(description = "content of the message")]
    content: String,
}

#[async_trait::async_trait]
impl VizierTool for SendTelegramMessage {
    type Input = SendTelegramMessageArgs;
    type Output = String;

    fn name() -> String {
        "telegram_send_message".to_string()
    }

    fn description(&self) -> String {
        "send a telegram message to a chat, avoid using this when user interact with you directly from telegram".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let chat_id = args.chat_id;
        let content = args.content.clone();

        crate::utils::telegram::send_message(&self.bot, ChatId(chat_id), args.content)
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        let channel = VizierChannelId::TelegramChannel(chat_id);
        let key = format!("{}__{}", self.agent_id, channel.to_slug());
        let topic_id = if let Ok(Some(value)) = self.storage.get_state(key).await {
            let state: ChannelState = serde_json::from_value(value).unwrap_or(ChannelState { active_topic: None });
            state.active_topic
        } else {
            None
        };

        let session = VizierSession(self.agent_id.clone(), channel, topic_id);
        let response = VizierResponse {
            timestamp: Utc::now(),
            content: VizierResponseContent::Message { content, stats: None },
            attachments: vec![],
        };
        self.storage
            .save_session_history(session, crate::schema::SessionHistoryContent::Response(response))
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        Ok(format!("Message sent to chat {}", chat_id))
    }
}

pub struct ReactTelegramMessage {
    bot: Bot,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ReactTelegramMessageArgs {
    #[schemars(description = "id of the target telegram chat")]
    chat_id: i64,

    #[schemars(description = "id of the target telegram message")]
    message_id: i64,

    #[schemars(description = "an emoji reaction")]
    emoji: String,
}

#[async_trait::async_trait]
impl VizierTool for ReactTelegramMessage {
    type Input = ReactTelegramMessageArgs;
    type Output = String;

    fn name() -> String {
        "telegram_react_message".to_string()
    }

    fn description(&self) -> String {
        "emoji react to a telegram message".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let chat_id = ChatId(args.chat_id);
        let message_id = teloxide::types::MessageId(args.message_id as i32);

        self.bot
            .send_message(chat_id, format!("Reaction: {}", args.emoji))
            .reply_to(message_id)
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        Ok(format!("Reacted with {} to message {}", args.emoji, args.message_id))
    }
}

pub struct GetTelegramMessage {
    bot: Bot,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GetTelegramMessageArgs {
    #[schemars(description = "id of the target telegram chat")]
    chat_id: i64,

    #[schemars(description = "id of the target telegram message")]
    message_id: i64,
}

#[async_trait::async_trait]
impl VizierTool for GetTelegramMessage {
    type Input = GetTelegramMessageArgs;
    type Output = String;

    fn name() -> String {
        "telegram_get_message_by_id".to_string()
    }

    fn description(&self) -> String {
        "get message by message id".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let chat_id = ChatId(args.chat_id);
        let message_id = teloxide::types::MessageId(args.message_id as i32);

        let response = self
            .bot
            .edit_message_text(chat_id, message_id, "Retrieving message...")
            .await;

        match response {
            Ok(msg) => Ok(format!(
                "{}: {}",
                msg.from
                    .clone()
                    .map(|u| u.full_name())
                    .unwrap_or_else(|| "Unknown".into()),
                msg.text().unwrap_or("")
            )),
            Err(err) => throw_vizier_error("telegram_get_message ", err),
        }
    }
}

