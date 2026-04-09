use rig::{completion::ToolDefinition, tool::Tool};
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use teloxide::{Bot, types::Recipient};

use crate::{config::VizierConfig, error::VizierError, utils::telegram::send_message};

pub struct TelegramDmPrimaryUser {
    bot: Option<Bot>,
    username: String,
}

impl TelegramDmPrimaryUser {
    pub fn new(config: Arc<VizierConfig>) -> Self {
        let bot_token = if let Some(telegram) = &config.channels.telegram {
            if let Some((_, telegram_config)) = telegram.iter().next() {
                telegram_config.token.clone()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let bot = if !bot_token.is_empty() {
            Some(Bot::new(bot_token))
        } else {
            None
        };

        let username = config.primary_user.telegram_username.clone();

        Self { bot, username }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TelegramDmPrimaryUserArgs {
    #[schemars(description = "content of the message")]
    content: String,
}

impl Tool for TelegramDmPrimaryUser
where
    Self: Sync + Send,
{
    const NAME: &'static str = "telegram_dm_primary_user";
    type Error = VizierError;
    type Args = TelegramDmPrimaryUserArgs;
    type Output = ();

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let parameters = serde_json::to_value(schema_for!(Self::Args)).unwrap();
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "send a direct message to the primary user on Telegram".into(),
            parameters,
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let bot = match &self.bot {
            Some(bot) => bot,
            None => {
                log::warn!("telegram_dm_primary_user: no bot token configured");
                return Ok(());
            }
        };

        if self.username.is_empty() {
            log::warn!("telegram_dm_primary_user: no telegram_username configured");
            return Ok(());
        }

        let username = if self.username.starts_with('@') {
            self.username.clone()
        } else {
            format!("@{}", self.username)
        };

        println!("{username}");
        let recipient = Recipient::ChannelUsername(username.clone());
        match send_message(bot, recipient, args.content).await {
            Ok(()) => Ok(()),
            Err(err) => {
                log::error!(
                    "telegram_dm_primary_user: failed to send message to {}: {:?}",
                    username,
                    err
                );
                Ok(())
            }
        }
    }
}
