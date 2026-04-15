use serde::{Deserialize, Serialize};
use std::sync::Arc;
use teloxide::{Bot, types::Recipient};

use crate::agents::tools::VizierTool;
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

#[async_trait::async_trait]
impl VizierTool for TelegramDmPrimaryUser
where
    Self: Sync + Send,
{
    type Input = TelegramDmPrimaryUserArgs;
    type Output = ();

    fn name() -> String {
        "telegram_dm_primary_user".to_string()
    }

    fn description(&self) -> String {
        "send a direct message to the primary user on Telegram".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
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
