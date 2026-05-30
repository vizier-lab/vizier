use serde::{Deserialize, Serialize};
use teloxide::{Bot, types::Recipient};

use crate::agents::tools::VizierTool;
use crate::{error::VizierError, utils::telegram::send_message};

pub struct TelegramDmPrimaryUser {
    bot: Option<Bot>,
    username: String,
}

impl TelegramDmPrimaryUser {
    pub fn new(token: Option<String>, username: String) -> Self {
        let bot = token.filter(|t| !t.is_empty()).map(Bot::new);

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
                tracing::warn!("telegram_dm_primary_user: no bot token configured");
                return Ok(());
            }
        };

        if self.username.is_empty() {
            tracing::warn!("telegram_dm_primary_user: no telegram_username configured");
            return Ok(());
        }

        let username = if self.username.starts_with('@') {
            self.username.clone()
        } else {
            format!("@{}", self.username)
        };

        let recipient = Recipient::ChannelUsername(username.clone());
        match send_message(bot, recipient, args.content).await {
            Ok(()) => Ok(()),
            Err(err) => {
                tracing::error!(
                    "telegram_dm_primary_user: failed to send message to {}: {:?}",
                    username,
                    err
                );
                Ok(())
            }
        }
    }
}
