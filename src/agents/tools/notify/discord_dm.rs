use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serenity::all::{Http, UserId};

use crate::{
    agents::tools::VizierTool, error::VizierError,
    utils::discord::send_message,
};

pub struct DiscordDmPrimaryUser {
    http: Option<Arc<Http>>,
    discord_id: u64,
}

impl DiscordDmPrimaryUser {
    pub fn new(token: Option<String>, discord_id: String) -> Self {
        let http = token.map(|t| Arc::new(Http::new(&t)));
        let discord_id = discord_id.parse::<u64>().unwrap_or(0);

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

        let http = match &self.http {
            Some(http) => http,
            None => {
                tracing::warn!("discord_dm_primary_user: no discord token configured");
                return Ok(());
            }
        };

        let user_id = UserId::new(self.discord_id);
        let channel_id = match user_id.create_dm_channel(http.clone()).await {
            Ok(channel) => channel.id,
            Err(err) => {
                tracing::error!(
                    "discord_dm_primary_user: failed to create DM channel: {:?}",
                    err
                );
                return Ok(());
            }
        };

        match send_message(http.clone(), &channel_id, args.content).await {
            Ok(()) => Ok(()),
            Err(err) => {
                tracing::error!("discord_dm_primary_user: failed to send message: {:?}", err);
                Ok(())
            }
        }
    }
}
