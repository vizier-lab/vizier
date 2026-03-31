use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use serenity::all::{
    ChannelId, Command, CreateCommand, CreateInteractionResponseMessage, Http, Interaction, Ready,
};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;

use crate::channels::VizierChannel;
use crate::config::DiscordChannelConfig;
use crate::schema::{
    VizierChannelId, VizierRequest, VizierRequestContent, VizierResponse, VizierSession,
};
use crate::transport::VizierTransport;
use crate::utils::remove_think_tags;

pub struct DiscordChannelReader {
    client: Client,
}

impl DiscordChannelReader {
    pub async fn new(
        agent_id: String,
        config: DiscordChannelConfig,
        transport: VizierTransport,
    ) -> Result<Self> {
        let intents = GatewayIntents::all();
        let token = config.token.clone();
        let client = Client::builder(token.clone(), intents)
            .event_handler(Handler(agent_id, transport.clone()))
            .await?;

        Ok(Self { client })
    }
}

impl VizierChannel for DiscordChannelReader {
    async fn run(&mut self) -> Result<()> {
        if let Err(err) = self.client.start().await {
            log::error!("{:?}", err);
        }
        Ok(())
    }
}

pub struct DiscordChannelWriter {
    transport: VizierTransport,
    config: HashMap<String, DiscordChannelConfig>,
}

impl DiscordChannelWriter {
    pub fn new(transport: VizierTransport, config: HashMap<String, DiscordChannelConfig>) -> Self {
        Self { transport, config }
    }
}

impl VizierChannel for DiscordChannelWriter {
    async fn run(&mut self) -> Result<()> {
        let mut token_map = HashMap::new();
        for (agent_id, config) in self.config.iter() {
            let token = config.token.clone();
            token_map.insert(agent_id.clone(), Arc::new(Http::new(&token)));
        }

        let mut recv = self.transport.subscribe_response().await?;
        let _ = tokio::spawn(async move {
            loop {
                if let Ok((
                    VizierSession(agent_id, VizierChannelId::DiscordChanel(channel_id), _),
                    res,
                )) = recv.recv().await
                {
                    let http = token_map.get(&agent_id).unwrap().clone();
                    let channel_id = ChannelId::new(channel_id);

                    match res {
                        VizierResponse::ThinkingProgress => {
                            tokio::spawn(async move {
                                let _ = channel_id.broadcast_typing(&http).await;
                            });
                        }
                        VizierResponse::Message { content, stats: _ } => {
                            let content = remove_think_tags(&content.clone());
                            let _ = crate::utils::discord::send_message(
                                http.clone(),
                                &channel_id,
                                content,
                            )
                            .await;
                        }

                        VizierResponse::Thinking { name, args } => {
                            let _ = crate::utils::discord::send_message(
                                http.clone(),
                                &channel_id,
                                crate::utils::format_thinking(&name, &args),
                            )
                            .await;
                        }
                        VizierResponse::Abort => {
                            let _ = crate::utils::discord::send_message(
                                http.clone(),
                                &channel_id,
                                "thinking aborted".into(),
                            )
                            .await;
                        }

                        _ => {}
                    }
                }
            }
        })
        .await;

        Ok(())
    }
}

struct Handler(String, VizierTransport);

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, _ready: Ready) {
        let ping = CreateCommand::new("ping").description("a simple ping");
        let lobotomy = CreateCommand::new("lobotomy")
            .description("Reset current conversation in this channel");
        let help = CreateCommand::new("help").description("How to use me");

        let _ = Command::create_global_command(ctx.http.clone(), ping).await;
        let _ = Command::create_global_command(ctx.http.clone(), lobotomy).await;
        let _ = Command::create_global_command(ctx.http.clone(), help).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            let agent_id = self.0.clone();

            if command.data.name == "ping" {
                let _ = command
                    .create_response(
                        ctx.http.clone(),
                        serenity::all::CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new().content("Pong!"),
                        ),
                    )
                    .await;
            }

            if command.data.name == "lobotomy" {
                let _ = command
                    .create_response(
                        ctx.http.clone(),
                        serenity::all::CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new().content("NOOOOOOOOO!!!"),
                        ),
                    )
                    .await;

                let metadata = json!({
                    "sent_at": Utc::now().to_string(),
                    "discord_channel_id": command.channel_id.to_string(),
                });

                if let Err(err) = self
                    .1
                    .send_request(
                        VizierSession(
                            agent_id,
                            VizierChannelId::DiscordChanel(command.channel_id.get()),
                            None,
                        ),
                        VizierRequest {
                            user: format!(
                                "@{} (DiscordId: {})",
                                command.user.display_name(),
                                command.user.id.to_string()
                            ),
                            content: VizierRequestContent::Command("/lobotomy".into()),
                            metadata,
                        },
                    )
                    .await
                {
                    log::error!("{}", err)
                }
            }

            if command.data.name == "help" {
                if let Err(err) = command
                    .create_response(
                        ctx.http.clone(),
                        serenity::all::CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new().content(
                                r#"
Just mention me when you need to summon me.
I will only read the chat otherwise.
If I am halucinating, feel free to `/lobotomy` me
                            "#,
                            ),
                        ),
                    )
                    .await
                {
                    log::error!("{}", err)
                }
            }
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if let Ok(is_mention) = msg.mentions_me(ctx.http).await {
            let agent_id = self.0.clone();
            let transport = self.1.clone();
            let current_user = ctx.cache.current_user().discriminator;
            if msg.author.discriminator == current_user {
                return;
            }

            let replied_to = match msg.referenced_message {
                None => None,
                Some(message) => Some(message.id.to_string()),
            };

            let metadata = json!({
                "sent_at": Utc::now().to_string(),
                "is_reply_message": replied_to.is_some(),
                "replied_message_id": replied_to,
                "message_id": msg.id.to_string(),
                "discord_channel_id": msg.channel_id.to_string(),
            });

            if !is_mention {
                tokio::spawn(async move {
                    if let Err(err) = transport
                        .send_request(
                            VizierSession(
                                agent_id,
                                VizierChannelId::DiscordChanel(msg.channel_id.get()),
                                None,
                            ),
                            VizierRequest {
                                user: format!(
                                    "@{} (DiscordId: {})",
                                    msg.author.display_name(),
                                    msg.author.id.to_string()
                                ),
                                content: VizierRequestContent::SilentRead(msg.content),
                                metadata,
                            },
                        )
                        .await
                    {
                        log::error!("{}", err)
                    }
                });

                return;
            }

            tokio::spawn(async move {
                if let Err(err) = transport
                    .send_request(
                        VizierSession(
                            agent_id,
                            VizierChannelId::DiscordChanel(msg.channel_id.get()),
                            None,
                        ),
                        VizierRequest {
                            user: format!(
                                "@{} (DiscordId: {})",
                                msg.author.display_name(),
                                msg.author.id.to_string()
                            ),
                            content: VizierRequestContent::Chat(msg.content),
                            metadata,
                        },
                    )
                    .await
                {
                    log::error!("{}", err)
                }
            });
        }
    }
}
