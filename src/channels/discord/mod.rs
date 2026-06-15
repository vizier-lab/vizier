use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serenity::all::{
    ChannelId, Command, CreateAttachment, CreateCommand, CreateCommandOption,
    CreateInteractionResponseMessage, CreateMessage, Http, Interaction, Ready, Typing,
};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;

use crate::channels::VizierChannel;
use crate::error::VizierError;
use crate::dependencies::VizierDependencies;
use crate::schema::{
    PlatformMessageId, TopicId, VizierAttachment, VizierAttachmentContent, VizierChannelId,
    VizierRequest, VizierRequestContent, VizierResponse, VizierResponseContent, VizierSession,
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
        token: String,
        deps: VizierDependencies,
    ) -> Result<Self> {
        let intents = GatewayIntents::all();
        let client = Client::builder(token.clone(), intents)
            .event_handler(Handler(agent_id, deps.clone()))
            .await?;

        Ok(Self { client })
    }
}

impl VizierChannel for DiscordChannelReader {
    async fn run(&mut self) -> Result<()> {
        if let Err(err) = self.client.start().await {
            tracing::error!("{:?}", err);
        }
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

        let new = CreateCommand::new("new").description("create fresh new session");
        let session = CreateCommand::new("session")
            .description("list or select session")
            .add_option(CreateCommandOption::new(
                serenity::all::CommandOptionType::String,
                "topic_id",
                "switch to the topic if not empty",
            ));

        let _ = Command::create_global_command(ctx.http.clone(), ping).await;

        let _ = Command::create_global_command(ctx.http.clone(), new).await;
        let _ = Command::create_global_command(ctx.http.clone(), session).await;

        let abort = CreateCommand::new("abort").description("abort current thinking");
        let _ = Command::create_global_command(ctx.http.clone(), abort).await;

        let checkpoint = CreateCommand::new("checkpoint").description("save checkpoint with handover summary");
        let _ = Command::create_global_command(ctx.http.clone(), checkpoint).await;

        let lobotomy = CreateCommand::new("lobotomy").description("save checkpoint without handover (clean break)");
        let _ = Command::create_global_command(ctx.http.clone(), lobotomy).await;
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
                    if let Ok(sessions) = self
                        .1
                        .storage
                        .get_session_list(agent_id.clone(), Some(channel))
                        .await
                    {
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

**Commands:**
• `/checkpoint` — Save checkpoint with handover summary
• `/lobotomy` — Save checkpoint without handover (clean break)
• `/abort` — Abort current thinking
• `/new` — Create new session
• `/session` — List or switch sessions
                            "#,
                            ),
                        ),
                    )
                    .await
                {
                    tracing::error!("{}", err)
                }
            }

            if command.data.name == "abort" {
                let channel = VizierChannelId::DiscordChanel(command.channel_id.get());
                let key = format!("{}__{}", agent_id, channel.to_slug());
                let topic_id = if let Ok(Some(value)) = self.1.storage.get_state(key).await {
                    serde_json::from_value::<ChannelState>(value)
                        .ok()
                        .and_then(|s| s.active_topic)
                } else {
                    None
                };

                let session = VizierSession(agent_id.clone(), channel, topic_id);
                let _ = self
                    .1
                    .transport
                    .send_request(
                        session,
                        VizierRequest {
                            timestamp: Utc::now(),
                            user: agent_id.clone(),
                            content: VizierRequestContent::Command("abort".to_string()),
                            platform_message_id: None,
                            metadata: serde_json::json!({}),
                            attachments: vec![],
                            expect_audio_reply: None,
                        },
                        None,
                    )
                    .await;

                let _ = command
                    .create_response(
                        ctx.http.clone(),
                        serenity::all::CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new().content("aborting..."),
                        ),
                    )
                    .await;
            }

            if command.data.name == "checkpoint" {
                let channel = VizierChannelId::DiscordChanel(command.channel_id.get());
                let key = format!("{}__{}", agent_id, channel.to_slug());
                let topic_id = if let Ok(Some(value)) = self.1.storage.get_state(key).await {
                    serde_json::from_value::<ChannelState>(value)
                        .ok()
                        .and_then(|s| s.active_topic)
                } else {
                    None
                };

                let session = VizierSession(agent_id.clone(), channel, topic_id);
                let _ = self
                    .1
                    .transport
                    .send_request(
                        session,
                        VizierRequest {
                            timestamp: Utc::now(),
                            user: agent_id.clone(),
                            content: VizierRequestContent::Command("checkpoint".to_string()),
                            platform_message_id: None,
                            metadata: serde_json::json!({}),
                            attachments: vec![],
                            expect_audio_reply: None,
                        },
                        None,
                    )
                    .await;

                let _ = command
                    .create_response(
                        ctx.http.clone(),
                        serenity::all::CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new().content("creating checkpoint..."),
                        ),
                    )
                    .await;
            }

            if command.data.name == "lobotomy" {
                let channel = VizierChannelId::DiscordChanel(command.channel_id.get());
                let key = format!("{}__{}", agent_id, channel.to_slug());
                let topic_id = if let Ok(Some(value)) = self.1.storage.get_state(key).await {
                    serde_json::from_value::<ChannelState>(value)
                        .ok()
                        .and_then(|s| s.active_topic)
                } else {
                    None
                };

                let session = VizierSession(agent_id.clone(), channel, topic_id);
                let _ = self
                    .1
                    .transport
                    .send_request(
                        session,
                        VizierRequest {
                            timestamp: Utc::now(),
                            user: agent_id.clone(),
                            content: VizierRequestContent::Command("lobotomy".to_string()),
                            platform_message_id: None,
                            metadata: serde_json::json!({}),
                            attachments: vec![],
                            expect_audio_reply: None,
                        },
                        None,
                    )
                    .await;

                let _ = command
                    .create_response(
                        ctx.http.clone(),
                        serenity::all::CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new().content("performing lobotomy..."),
                        ),
                    )
                    .await;
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

        let is_dm = msg.guild_id.is_none();

        if let Ok(is_mention) = msg.mentions_me(&ctx.http).await {
            let mut attachments = vec![];
            for attachment in &msg.attachments {
                let bytes_result = async {
                    let resp = reqwest::get(&attachment.url).await?;
                    resp.bytes().await
                }.await;
                if let Ok(bytes) = bytes_result {
                    if let Ok(file_record) = self.1.transport.send_file_upload(attachment.filename.clone(), bytes.to_vec()).await {
                        attachments.push(VizierAttachment {
                            filename: attachment.filename.clone(),
                            content: VizierAttachmentContent::Local(file_record.url),
                        });
                    }
                }
            }

            let agent_id = self.0.clone();
            let transport = self.1.transport.clone();
            let file_manager = self.1.file_manager.clone();
            let http = ctx.http.clone();
            let current_user = ctx.cache.current_user().discriminator;
            if msg.author.discriminator == current_user {
                return;
            }
            let bot_name = ctx.cache.current_user().name.clone();

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
                "is_dm": is_dm,
            });

            let session = VizierSession(
                agent_id.clone(),
                VizierChannelId::DiscordChanel(msg.channel_id.get()),
                topic_id,
            );

            let (content, request_content) = if !is_mention && !is_dm {
                (
                    msg.content.clone(),
                    VizierRequestContent::SilentRead(msg.content),
                )
            } else {
                let cleaned = if is_mention {
                    msg.content
                        .replace(&format!("@{}", bot_name), "")
                        .trim()
                        .to_string()
                } else {
                    msg.content.clone()
                };
                (cleaned.clone(), VizierRequestContent::Chat(cleaned))
            };

            let request = VizierRequest {
                timestamp: chrono::Utc::now(),
                user: format!(
                    "@{} (DiscordId: {})",
                    msg.author.display_name(),
                    msg.author.id.to_string()
                ),
                content: request_content,
                platform_message_id: Some(PlatformMessageId::Discord(msg.id.get())),
                metadata,
                attachments,
                ..Default::default()
            };

            let discord_channel_id = ChannelId::new(msg.channel_id.get());
            let is_chat = matches!(request.content, VizierRequestContent::Chat(_));

            tokio::spawn(async move {
                let (response_tx, response_rx) = flume::unbounded();

                if let Err(err) = transport
                    .send_request(session.clone(), request, Some(response_tx))
                    .await
                {
                    tracing::error!("{}", err);
                    return;
                }

                let mut typing_state: Option<Typing> = None;

                while let Ok(response) = response_rx.recv_async().await {
                    match response {
                        VizierResponse {
                            content: VizierResponseContent::ThinkingStart,
                            ..
                        } => {
                            typing_state = Some(Typing::start(http.clone(), discord_channel_id));
                        }
                        VizierResponse {
                            content: VizierResponseContent::ToolChoice { name, args },
                            ..
                        } => {
                            let _ = crate::utils::discord::send_message(
                                http.clone(),
                                &discord_channel_id,
                                crate::utils::format_thinking(&name, &args),
                            )
                            .await;
                        }
                        VizierResponse {
                            content: VizierResponseContent::Thinking(thought),
                            ..
                        } => {
                            let _ = crate::utils::discord::send_message(
                                http.clone(),
                                &discord_channel_id,
                                format!("> {}", thought),
                            )
                            .await;
                        }
                        VizierResponse {
                            content:
                                VizierResponseContent::Message {
                                    content, stats: _
                                },
                            attachments,
                            ..
                        } => {
                            if let Some(typing) = typing_state.take() {
                                typing.stop();
                            }
                            let content = remove_think_tags(&content);
                            let _ = crate::utils::discord::send_message(
                                http.clone(),
                                &discord_channel_id,
                                content,
                            )
                            .await;

                            for attachment in &attachments {
                                match file_manager.resolve(attachment).await {
                                    Ok((filename, bytes)) => {
                                        let files = vec![CreateAttachment::bytes(bytes, &filename)];
                                        let builder = CreateMessage::new();
                                        if let Err(err) = discord_channel_id
                                            .send_files(&http, files, builder)
                                            .await
                                        {
                                            tracing::error!(
                                                "Failed to send attachment {}: {:?}",
                                                filename,
                                                err
                                            );
                                        }
                                    }
                                    Err(err) => {
                                        tracing::error!(
                                            "Failed to resolve attachment {:?}: {:?}",
                                            attachment.filename,
                                            err
                                        );
                                    }
                                }
                            }
                        }
                        VizierResponse {
                            content:
                                VizierResponseContent::AudioReply(audio_att, text, _),
                            ..
                        } => {
                            if let Some(typing) = typing_state.take() {
                                typing.stop();
                            }
                            if let Some(content) = text {
                                let content = remove_think_tags(&content);
                                let _ = crate::utils::discord::send_message(
                                    http.clone(),
                                    &discord_channel_id,
                                    content,
                                )
                                .await;
                            }
                            match file_manager.resolve(&audio_att).await {
                                Ok((filename, bytes)) => {
                                    let files = vec![CreateAttachment::bytes(bytes, &filename)];
                                    let builder = CreateMessage::new();
                                    if let Err(err) = discord_channel_id
                                        .send_files(&http, files, builder)
                                        .await
                                    {
                                        tracing::error!(
                                            "Failed to send audio reply: {:?}",
                                            err
                                        );
                                    }
                                }
                                Err(err) => {
                                    tracing::error!(
                                        "Failed to resolve audio reply {:?}: {:?}",
                                        audio_att.filename,
                                        err
                                    );
                                }
                            }
                        }
                        VizierResponse {
                            content: VizierResponseContent::Abort,
                            ..
                        } => {
                            if let Some(typing) = typing_state.take() {
                                typing.stop();
                            }
                            let _ = crate::utils::discord::send_message(
                                http.clone(),
                                &discord_channel_id,
                                "thinking aborted".into(),
                            )
                            .await;
                        }
                        VizierResponse {
                            content: VizierResponseContent::Error { kind, message },
                            ..
                        } => {
                            if let Some(typing) = typing_state.take() {
                                typing.stop();
                            }
                            let kind_str = match kind {
                                crate::schema::ErrorKind::Completion => "Completion Error",
                                crate::schema::ErrorKind::ToolTimeout => "Tool Timeout",
                                crate::schema::ErrorKind::PromptTimeout => "Prompt Timeout",
                            };
                            let _ = crate::utils::discord::send_message(
                                http.clone(),
                                &discord_channel_id,
                                format!("**{}**: {}", kind_str, message),
                            )
                            .await;
                        }
                    _ => {}
                }
            }

            if let Some(typing) = typing_state.take() {
                typing.stop();
            }
        });
        }
    }
}
