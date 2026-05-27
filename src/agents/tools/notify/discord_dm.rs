use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serenity::all::{Http, UserId};

use crate::{
    agents::tools::VizierTool, config::VizierConfig, error::VizierError,
    utils::discord::send_message,
};

pub struct DiscordDmPrimaryUser {
    http: Arc<Http>,
    discord_id: u64,
}

impl DiscordDmPrimaryUser {
    pub fn new(config: Arc<VizierConfig>) -> Self {
        let http = if let Some(discord) = &config.channels.discord {
            if let Some((_, discord_config)) = discord.iter().next() {
                Arc::new(Http::new(&discord_config.token))
            } else {
                Arc::new(Http::new(""))
            }
        } else {
            Arc::new(Http::new(""))
        };

        let discord_id = config.primary_user.discord_id.parse::<u64>().unwrap_or(0);

        Self { http, discord_id }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DiscordDmPrimaryUserArgs {
    #[schemars(description = "content of the message")]
    content: String,
}

#[async_trait::async_trait]
impl VizierTool for DiscordDmPrimaryUser
where
    Self: Sync + Send,
{
    type Input = DiscordDmPrimaryUserArgs;
    type Output = ();

    fn name() -> String {
        "discord_dm_primary_user".to_string()
    }

    fn description(&self) -> String {
        "send a direct message to the primary user on Discord".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        if self.discord_id == 0 {
            tracing::warn!("discord_dm_primary_user: no discord_id configured");
            return Ok(());
        }

        let user_id = UserId::new(self.discord_id);
        let channel_id = match user_id.create_dm_channel(self.http.clone()).await {
            Ok(channel) => channel.id,
            Err(err) => {
                tracing::error!(
                    "discord_dm_primary_user: failed to create DM channel: {:?}",
                    err
                );
                return Ok(());
            }
        };

        match send_message(self.http.clone(), &channel_id, args.content).await {
            Ok(()) => Ok(()),
            Err(err) => {
                tracing::error!("discord_dm_primary_user: failed to send message: {:?}", err);
                Ok(())
            }
        }
    }
}
