use serde::{Deserialize, Serialize};
use teloxide::Bot;
use teloxide::prelude::*;
use teloxide::sugar::request::RequestReplyExt;

use crate::agents::tools::VizierTool;
use crate::error::{VizierError, throw_vizier_error};

pub fn new_telegram_tools(
    bot_token: String,
) -> (
    SendTelegramMessage,
    ReactTelegramMessage,
    GetTelegramMessage,
) {
    let bot = Bot::new(bot_token);

    (
        SendTelegramMessage { bot: bot.clone() },
        ReactTelegramMessage { bot: bot.clone() },
        GetTelegramMessage { bot: bot.clone() },
    )
}

pub struct SendTelegramMessage {
    bot: Bot,
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
    type Output = ();

    fn name() -> String {
        "telegram_send_message".to_string()
    }

    fn description(&self) -> String {
        "send a telegram message to a chat, avoid using this when user interact with you directly from telegram".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let response =
            crate::utils::telegram::send_message(&self.bot, ChatId(args.chat_id), args.content)
                .await;

        match response {
            Ok(()) => Ok(()),
            Err(err) => throw_vizier_error("telegram_send_message ", err),
        }
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
    type Output = ();

    fn name() -> String {
        "telegram_react_message".to_string()
    }

    fn description(&self) -> String {
        "emoji react to a telegram message".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let chat_id = ChatId(args.chat_id);
        let message_id = teloxide::types::MessageId(args.message_id as i32);

        let response = self
            .bot
            .send_message(chat_id, format!("Reaction: {}", args.emoji))
            .reply_to(message_id)
            .await;

        match response {
            Ok(_) => Ok(()),
            Err(err) => throw_vizier_error("telegram_react_message ", err),
        }
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

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
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

