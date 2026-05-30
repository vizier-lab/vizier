use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::VizierTool,
    config::user::UserConfig,
    error::VizierError,
    schema::{VizierChannelId, VizierResponseContent, VizierSession},
    transport::VizierTransport,
};

pub struct NotifyPrimaryUser {
    discord_token: Option<String>,
    telegram_token: Option<String>,
    primary_user: UserConfig,
    agent_id: String,
    transport: VizierTransport,
}

impl NotifyPrimaryUser {
    pub fn new(
        discord_token: Option<String>,
        telegram_token: Option<String>,
        primary_user: UserConfig,
        agent_id: String,
        transport: VizierTransport,
    ) -> Self {
        Self {
            discord_token,
            telegram_token,
            primary_user,
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

        let discord_token = self.discord_token.clone();
        let discord_id = self.primary_user.discord_id.clone();
        let content2 = content.clone();

        let telegram_token = self.telegram_token.clone();
        let telegram_username = self.primary_user.telegram_username.clone();
        let content3 = content.clone();

        let agent_id = self.agent_id.clone();
        let transport = self.transport.clone();
        let content4 = content.clone();

        let discord_handle = tokio::spawn(async move {
            Self::send_discord_internal(discord_token, discord_id, &content).await;
        });
        let telegram_handle = tokio::spawn(async move {
            Self::send_telegram_internal(telegram_token, telegram_username, &content2).await;
        });
        let webui_handle = tokio::spawn(async move {
            Self::send_webui_internal(&agent_id, &transport, &content4).await;
        });

        let _ = tokio::join!(discord_handle, telegram_handle, webui_handle);

        Ok(())
    }
}

impl NotifyPrimaryUser {
    async fn send_discord_internal(token: Option<String>, discord_id: String, content: &str) {
        use crate::utils::discord::send_message;
        use serenity::all::{Http, UserId};
        use std::sync::Arc;

        let token = match token {
            Some(t) => t,
            None => return,
        };

        let discord_id = match discord_id.parse::<u64>() {
            Ok(id) => id,
            Err(_) => {
                tracing::warn!("notify_primary_user: no discord_id configured");
                return;
            }
        };

        let http = Arc::new(Http::new(&token));

        let user_id = UserId::new(discord_id);
        let channel_id = match user_id.create_dm_channel(http.clone()).await {
            Ok(channel) => channel.id,
            Err(err) => {
                tracing::error!(
                    "notify_primary_user: failed to create Discord DM channel: {:?}",
                    err
                );
                return;
            }
        };

        if let Err(err) = send_message(http, &channel_id, content.to_string()).await {
            tracing::error!(
                "notify_primary_user: failed to send Discord message: {:?}",
                err
            );
        }
    }

    async fn send_telegram_internal(token: Option<String>, username: String, content: &str) {
        use crate::utils::telegram::send_message;
        use teloxide::Bot;

        let token = match token {
            Some(t) => t,
            None => return,
        };

        let bot = Bot::new(token);

        if username.is_empty() {
            tracing::warn!("notify_primary_user: no telegram_username configured");
            return;
        }

        let username = if username.starts_with('@') {
            username
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
            tracing::error!(
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
            attachments: vec![],
        };

        if let Err(err) = transport.send_response(session, response).await {
            tracing::error!(
                "notify_primary_user: failed to send WebUI notification: {:?}",
                err
            );
        }
    }
}
