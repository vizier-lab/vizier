use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::VizierTool,
    config::VizierConfig,
    error::VizierError,
    schema::{VizierChannelId, VizierResponseContent, VizierSession},
    transport::VizierTransport,
};

pub struct NotifyPrimaryUser {
    config: Arc<VizierConfig>,
    agent_id: String,
    transport: VizierTransport,
}

impl NotifyPrimaryUser {
    pub fn new(config: Arc<VizierConfig>, agent_id: String, transport: VizierTransport) -> Self {
        Self {
            config,
            agent_id,
            transport,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct NotifyPrimaryUserArgs {
    #[schemars(description = "content of the notification")]
    content: String,
}

#[async_trait::async_trait]
impl VizierTool for NotifyPrimaryUser
where
    Self: Sync + Send,
{
    type Input = NotifyPrimaryUserArgs;
    type Output = ();

    fn name() -> String {
        "notify_primary_user".to_string()
    }

    fn description(&self) -> String {
        "send a notification to the primary user via all available channels (Discord, Telegram, WebUI)".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let content = args.content;
        let config = self.config.clone();
        let config2 = config.clone();
        let agent_id = self.agent_id.clone();
        let agent_id2 = agent_id.clone();
        let transport = self.transport.clone();
        let transport2 = transport.clone();
        let content2 = content.clone();
        let content3 = content.clone();

        let discord_handle = tokio::spawn(async move {
            Self::send_discord_internal(&config, &content).await;
        });
        let telegram_handle = tokio::spawn(async move {
            Self::send_telegram_internal(&config2, &content2).await;
        });
        let webui_handle = tokio::spawn(async move {
            Self::send_webui_internal(&agent_id2, &transport2, &content3).await;
        });

        let _ = tokio::join!(discord_handle, telegram_handle, webui_handle);

        Ok(())
    }
}

impl NotifyPrimaryUser {
    async fn send_discord_internal(config: &Arc<VizierConfig>, content: &str) {
        use crate::utils::discord::send_message;
        use serenity::all::{Http, UserId};
        use std::sync::Arc;

        let discord_id = match config.primary_user.discord_id.parse::<u64>() {
            Ok(id) => id,
            Err(_) => {
                log::warn!("notify_primary_user: no discord_id configured");
                return;
            }
        };

        let http = if let Some(discord) = &config.channels.discord {
            if let Some((_, discord_config)) = discord.iter().next() {
                Arc::new(Http::new(&discord_config.token))
            } else {
                return;
            }
        } else {
            return;
        };

        let user_id = UserId::new(discord_id);
        let channel_id = match user_id.create_dm_channel(http.clone()).await {
            Ok(channel) => channel.id,
            Err(err) => {
                log::error!(
                    "notify_primary_user: failed to create Discord DM channel: {:?}",
                    err
                );
                return;
            }
        };

        if let Err(err) = send_message(http, &channel_id, content.to_string()).await {
            log::error!(
                "notify_primary_user: failed to send Discord message: {:?}",
                err
            );
        }
    }

    async fn send_telegram_internal(config: &Arc<VizierConfig>, content: &str) {
        use crate::utils::telegram::send_message;
        use teloxide::Bot;

        let bot_token = if let Some(telegram) = &config.channels.telegram {
            if let Some((_, telegram_config)) = telegram.iter().next() {
                telegram_config.token.clone()
            } else {
                return;
            }
        } else {
            return;
        };

        let bot = Bot::new(bot_token);

        let username = &config.primary_user.telegram_username;
        if username.is_empty() {
            log::warn!("notify_primary_user: no telegram_username configured");
            return;
        }

        let username = if username.starts_with('@') {
            username.clone()
        } else {
            format!("@{}", username)
        };

        if let Err(err) = send_message(
            &bot,
            teloxide::types::Recipient::ChannelUsername(username.clone()),
            content.to_string(),
        )
        .await
        {
            log::error!(
                "notify_primary_user: failed to send Telegram message to {}: {:?}",
                username,
                err
            );
        }
    }

    async fn send_webui_internal(agent_id: &str, transport: &VizierTransport, content: &str) {
        use crate::schema::VizierResponse;

        let session = VizierSession(
            agent_id.to_string(),
            VizierChannelId::HTTP("vizier-webui".to_string()),
            Some("notification".to_string()),
        );

        let response = VizierResponse {
            timestamp: chrono::Utc::now(),
            content: VizierResponseContent::Message {
                content: content.to_string(),
                stats: None,
            },
        };

        if let Err(err) = transport.send_response(session, response).await {
            log::error!(
                "notify_primary_user: failed to send WebUI notification: {:?}",
                err
            );
        }
    }
}
