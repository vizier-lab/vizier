use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serenity::all::{
    ChannelId, Command, CreateCommand, CreateCommandOption, CreateInteractionResponseMessage, Http,
    Interaction, Ready, Typing,
};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;

use crate::channels::VizierChannel;
use crate::config::DiscordChannelConfig;
use crate::dependencies::VizierDependencies;
use crate::schema::{
    TopicId, VizierChannelId, VizierRequest, VizierRequestContent, VizierResponse, VizierSession,
};
use crate::storage::session::SessionStorage;
use crate::storage::state::StateStorage;
use crate::transport::VizierTransport;
use crate::utils::remove_think_tags;

pub struct DiscordChannelReader {
    client: Client,
}

impl DiscordChannelReader {
    pub async fn new(
        agent_id: String,
        config: DiscordChannelConfig,
        deps: VizierDependencies,
    ) -> Result<Self> {
        let intents = GatewayIntents::all();
        let token = config.token.clone();
        let client = Client::builder(token.clone(), intents)
            .event_handler(Handler(agent_id, deps.clone()))
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
            let mut typing_state = HashMap::<u64, Typing>::new();
            loop {
                if let Ok((
                    VizierSession(agent_id, VizierChannelId::DiscordChanel(channel_id), _),
                    res,
                )) = recv.recv().await
                {
                    let http = token_map.get(&agent_id).unwrap().clone();
                    let discord_channel_id = ChannelId::new(channel_id);

                    match res {
                        VizierResponse::ThinkingStart => {
                            typing_state.insert(
                                channel_id,
                                Typing::start(http.clone(), discord_channel_id),
                            );
                        }
                        VizierResponse::Thinking { name, args } => {
                            let _ = crate::utils::discord::send_message(
                                http.clone(),
                                &discord_channel_id,
                                crate::utils::format_thinking(&name, &args),
                            )
                            .await;
                        }
                        VizierResponse::Message { content, stats: _ } => {
                            if let Some(typing) = typing_state.remove(&channel_id) {
                                typing.stop();
                            }

                            let content = remove_think_tags(&content.clone());
                            let _ = crate::utils::discord::send_message(
                                http.clone(),
                                &discord_channel_id,
                                content,
                            )
                            .await;
                        }
                        VizierResponse::Abort => {
                            if let Some(typing) = typing_state.remove(&channel_id) {
                                typing.stop();
                            }

                            let _ = crate::utils::discord::send_message(
                                http.clone(),
                                &discord_channel_id,
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

struct Handler(String, VizierDependencies);

#[derive(Debug, Deserialize, Serialize)]
struct ChannelState {
    active_topic: Option<TopicId>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, _ready: Ready) {
        let ping = CreateCommand::new("ping").description("a simple ping");
        // let help = CreateCommand::new("help").description("How to use me");

        let new = CreateCommand::new("new").description("create fresh new session");
        let session = CreateCommand::new("session")
            .description("list or select session")
            .add_option(CreateCommandOption::new(
                serenity::all::CommandOptionType::String,
                "topic_id",
                "switch to the topic if not empty",
            ));

        let _ = Command::create_global_command(ctx.http.clone(), ping).await;
        // let _ = Command::create_global_command(ctx.http.clone(), help).await;

        let _ = Command::create_global_command(ctx.http.clone(), new).await;
        let _ = Command::create_global_command(ctx.http.clone(), session).await;
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

            if command.data.name == "new" {
                let channel = VizierChannelId::DiscordChanel(command.channel_id.get());
                let topic_id = nanoid::nanoid!(10);

                let _ = self
                    .1
                    .storage
                    .save_state(
                        format!("{}__{}", agent_id, channel.to_slug()),
                        serde_json::to_value(ChannelState {
                            active_topic: Some(topic_id.clone()),
                        })
                        .unwrap(),
                    )
                    .await;

                let _ = command
                    .create_response(
                        ctx.http.clone(),
                        serenity::all::CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!("switch to new session: **{}**", topic_id)),
                        ),
                    )
                    .await;
            }

            if command.data.name == "session" {
                let channel = VizierChannelId::DiscordChanel(command.channel_id.get());
                let opt = command.data.options.clone();

                if let Some(raw_topic_id) = opt.iter().find_map(|opt| {
                    if opt.name == "topic_id".to_string() {
                        Some(opt.value.as_str().unwrap().to_string())
                    } else {
                        None
                    }
                }) {
                    let topic_id: Option<TopicId> = if raw_topic_id == "DEFAULT".to_string() {
                        None
                    } else {
                        Some(raw_topic_id.clone())
                    };

                    if let Ok(Some(_)) = self
                        .1
                        .storage
                        .get_session_detail_by_topic(
                            agent_id.clone(),
                            channel.clone(),
                            topic_id.clone(),
                        )
                        .await
                    {
                        let _ = self
                            .1
                            .storage
                            .save_state(
                                format!("{}__{}", agent_id, channel.to_slug()),
                                serde_json::to_value(ChannelState {
                                    active_topic: topic_id,
                                })
                                .unwrap(),
                            )
                            .await;

                        let _ = command
                            .create_response(
                                ctx.http.clone(),
                                serenity::all::CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new().content(format!(
                                        "switch to session: **{}**",
                                        raw_topic_id
                                    )),
                                ),
                            )
                            .await;
                    } else {
                        let _ = command
                            .create_response(
                                ctx.http.clone(),
                                serenity::all::CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("topic not found"),
                                ),
                            )
                            .await;
                    }
                } else {
                    if let Ok(sessions) = self.1.storage.get_session_list(agent_id, channel).await {
                        let mut res = vec![];
                        for session in &sessions {
                            res.push(format!(
                                "topic_id: {}\ntitle: {}",
                                session.topic.clone().unwrap_or("DEFAULT".into()),
                                session.title.clone()
                            ));
                        }

                        let output = res.join("\n\n");
                        let _ = command
                            .create_response(
                                ctx.http.clone(),
                                serenity::all::CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new().content(output),
                                ),
                            )
                            .await;
                    }
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
        let agent_id = self.0.clone();
        let channel = VizierChannelId::DiscordChanel(msg.channel_id.get());

        let key = format!("{}__{}", agent_id, channel.to_slug());
        let topic_id = if let Ok(Some(value)) = self.1.storage.get_state(key).await {
            let state = serde_json::from_value::<ChannelState>(value).unwrap();

            state.active_topic
        } else {
            None
        };

        if let Ok(is_mention) = msg.mentions_me(ctx.http).await {
            let agent_id = self.0.clone();
            let transport = self.1.transport.clone();
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
                                topic_id,
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
                            topic_id,
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
