use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serenity::all::{
    ChannelId, Command, CreateAttachment, CreateCommand, CreateCommandOption,
    CreateInteractionResponseFollowup, CreateInteractionResponseMessage, CreateMessage, Http,
    Interaction, Ready, Typing,
};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;
use tokio::sync::watch;

use crate::channels::VizierChannel;
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::{
    PlatformMessageId, TopicId, VizierAttachment, VizierAttachmentContent, VizierChannelId,
    VizierRequest, VizierRequestContent, VizierResponse, VizierResponseContent, VizierSession,
};
use crate::storage::session::SessionStorage;
use crate::storage::state::StateStorage;
use crate::transport::VizierTransport;
use crate::utils::remove_think_tags;

pub struct DiscordChannelReader {
    deps: VizierDependencies,
    token: String,
    agent_id: String,
    shutdown: (flume::Sender<bool>, flume::Receiver<bool>),
}

impl DiscordChannelReader {
    pub async fn new(agent_id: String, token: String, deps: VizierDependencies) -> Result<Self> {
        Ok(Self {
            deps,
            agent_id,
            token,
            shutdown: flume::bounded(1),
        })
    }
}

#[async_trait::async_trait]
impl VizierChannel for DiscordChannelReader {
    async fn run(&self) -> Result<()> {
        let intents = GatewayIntents::all();
        let mut client = Client::builder(self.token.clone(), intents)
            .event_handler(Handler(self.agent_id.clone(), self.deps.clone()))
            .await?;
        let shard_manager = client.shard_manager.clone();

        let handle = tokio::spawn(async move {
            let result = client.start();
            result.await
        });

        let shutdown = self.shutdown.clone();
        tokio::spawn(async move {
            if shutdown.1.recv_async().await.is_ok() {
                shard_manager.shutdown_all().await;
            }
        });

        Ok(handle.await??)
    }

    async fn shutdown(&self) -> Result<()> {
        let res = self.shutdown.0.send_async(true).await;
        Ok(())
    }
}

struct Handler(String, VizierDependencies);

#[derive(Debug, Deserialize, Serialize)]
struct ChannelState {
    active_topic: Option<TopicId>,
    #[serde(default)]
    show_thinking: bool,
    #[serde(default)]
    show_tool_calls: bool,
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

        let checkpoint =
            CreateCommand::new("checkpoint").description("save checkpoint with handover summary");
        let _ = Command::create_global_command(ctx.http.clone(), checkpoint).await;

        let lobotomy = CreateCommand::new("lobotomy")
            .description("save checkpoint without handover (clean break)");
        let _ = Command::create_global_command(ctx.http.clone(), lobotomy).await;

        let thinking = CreateCommand::new("thinking").description("toggle showing thinking output");
        let _ = Command::create_global_command(ctx.http.clone(), thinking).await;

        let tool_calls = CreateCommand::new("tool_calls").description("toggle showing tool call details");
        let _ = Command::create_global_command(ctx.http.clone(), tool_calls).await;
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
                            show_thinking: false,
                            show_tool_calls: false,
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
                        let key = format!("{}__{}", agent_id, channel.to_slug());
                        let existing_state = if let Ok(Some(value)) = self.1.storage.get_state(key.clone()).await {
                            serde_json::from_value::<ChannelState>(value).unwrap_or(ChannelState {
                                active_topic: None,
                                show_thinking: false,
                                show_tool_calls: false,
                            })
                        } else {
                            ChannelState {
                                active_topic: None,
                                show_thinking: false,
                                show_tool_calls: false,
                            }
                        };
                        let _ = self
                            .1
                            .storage
                            .save_state(
                                key,
                                serde_json::to_value(ChannelState {
                                    active_topic: topic_id,
                                    show_thinking: existing_state.show_thinking,
                                    show_tool_calls: existing_state.show_tool_calls,
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
                let (response_tx, response_rx) = flume::unbounded();
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
                        Some(response_tx),
                    )
                    .await;

                let _ = command
                    .create_response(
                        ctx.http.clone(),
                        serenity::all::CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("creating checkpoint..."),
                        ),
                    )
                    .await;

                let followup_command = command.clone();
                let followup_http = ctx.http.clone();
                tokio::spawn(async move {
                    while let Ok(response) = response_rx.recv_async().await {
                        match response.content {
                            VizierResponseContent::Checkpoint { handover: Some(_) } => {
                                let _ = followup_command
                                    .create_followup(
                                        followup_http.clone(),
                                        CreateInteractionResponseFollowup::new()
                                            .content("✅ checkpoint saved"),
                                    )
                                    .await;
                                break;
                            }
                            VizierResponseContent::Checkpoint { handover: None } => {
                                let _ = followup_command
                                    .create_followup(
                                        followup_http.clone(),
                                        CreateInteractionResponseFollowup::new()
                                            .content("✅ lobotomy performed"),
                                    )
                                    .await;
                                break;
                            }
                            VizierResponseContent::Error { kind, message } => {
                                let kind_str = match kind {
                                    crate::schema::ErrorKind::Completion => "Completion Error",
                                    crate::schema::ErrorKind::ToolTimeout => "Tool Timeout",
                                    crate::schema::ErrorKind::PromptTimeout => "Prompt Timeout",
                                };
                                let _ = followup_command
                                    .create_followup(
                                        followup_http.clone(),
                                        CreateInteractionResponseFollowup::new().content(format!(
                                            "**{}**: {}",
                                            kind_str, message
                                        )),
                                    )
                                    .await;
                                break;
                            }
                            _ => {}
                        }
                    }
                });
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
                let (response_tx, response_rx) = flume::unbounded();
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
                        Some(response_tx),
                    )
                    .await;

                let _ = command
                    .create_response(
                        ctx.http.clone(),
                        serenity::all::CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("performing lobotomy..."),
                        ),
                    )
                    .await;

                let followup_command = command.clone();
                let followup_http = ctx.http.clone();
                tokio::spawn(async move {
                    while let Ok(response) = response_rx.recv_async().await {
                        match response.content {
                            VizierResponseContent::Checkpoint { handover: Some(_) } => {
                                let _ = followup_command
                                    .create_followup(
                                        followup_http.clone(),
                                        CreateInteractionResponseFollowup::new()
                                            .content("✅ checkpoint saved"),
                                    )
                                    .await;
                                break;
                            }
                            VizierResponseContent::Checkpoint { handover: None } => {
                                let _ = followup_command
                                    .create_followup(
                                        followup_http.clone(),
                                        CreateInteractionResponseFollowup::new()
                                            .content("✅ lobotomy performed"),
                                    )
                                    .await;
                                break;
                            }
                            VizierResponseContent::Error { kind, message } => {
                                let kind_str = match kind {
                                    crate::schema::ErrorKind::Completion => "Completion Error",
                                    crate::schema::ErrorKind::ToolTimeout => "Tool Timeout",
                                    crate::schema::ErrorKind::PromptTimeout => "Prompt Timeout",
                                };
                                let _ = followup_command
                                    .create_followup(
                                        followup_http.clone(),
                                        CreateInteractionResponseFollowup::new().content(format!(
                                            "**{}**: {}",
                                            kind_str, message
                                        )),
                                    )
                                    .await;
                                break;
                            }
                            _ => {}
                        }
                    }
                });
            }

            if command.data.name == "thinking" {
                let channel = VizierChannelId::DiscordChanel(command.channel_id.get());
                let key = format!("{}__{}", agent_id, channel.to_slug());
                let mut state = if let Ok(Some(value)) = self.1.storage.get_state(key.clone()).await {
                    serde_json::from_value::<ChannelState>(value).unwrap_or(ChannelState {
                        active_topic: None,
                        show_thinking: false,
                        show_tool_calls: false,
                    })
                } else {
                    ChannelState {
                        active_topic: None,
                        show_thinking: false,
                        show_tool_calls: false,
                    }
                };
                state.show_thinking = !state.show_thinking;
                let _ = self.1.storage.save_state(key, serde_json::to_value(&state).unwrap()).await;
                let status = if state.show_thinking { "ON" } else { "OFF" };
                let _ = command
                    .create_response(
                        ctx.http.clone(),
                        serenity::all::CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!("thinking output: **{}**", status)),
                        ),
                    )
                    .await;
            }

            if command.data.name == "tool_calls" {
                let channel = VizierChannelId::DiscordChanel(command.channel_id.get());
                let key = format!("{}__{}", agent_id, channel.to_slug());
                let mut state = if let Ok(Some(value)) = self.1.storage.get_state(key.clone()).await {
                    serde_json::from_value::<ChannelState>(value).unwrap_or(ChannelState {
                        active_topic: None,
                        show_thinking: false,
                        show_tool_calls: false,
                    })
                } else {
                    ChannelState {
                        active_topic: None,
                        show_thinking: false,
                        show_tool_calls: false,
                    }
                };
                state.show_tool_calls = !state.show_tool_calls;
                let _ = self.1.storage.save_state(key, serde_json::to_value(&state).unwrap()).await;
                let status = if state.show_tool_calls { "ON" } else { "OFF" };
                let _ = command
                    .create_response(
                        ctx.http.clone(),
                        serenity::all::CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!("tool call details: **{}**", status)),
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
        let (topic_id, show_thinking, show_tool_calls) = if let Ok(Some(value)) = self.1.storage.get_state(key).await {
            if let Ok(state) = serde_json::from_value::<ChannelState>(value) {
                (state.active_topic, state.show_thinking, state.show_tool_calls)
            } else {
                (None, false, false)
            }
        } else {
            (None, false, false)
        };

        let is_dm = msg.guild_id.is_none();

        if let Ok(is_mention) = msg.mentions_me(&ctx.http).await {
            let mut attachments = vec![];
            for attachment in &msg.attachments {
                let bytes_result = async {
                    let resp = reqwest::get(&attachment.url).await?;
                    resp.bytes().await
                }
                .await;
                if let Ok(bytes) = bytes_result {
                    if let Ok(file_record) = self
                        .1
                        .transport
                        .send_file_upload(attachment.filename.clone(), bytes.to_vec())
                        .await
                    {
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
                            if show_tool_calls {
                                let _ = crate::utils::discord::send_message(
                                    http.clone(),
                                    &discord_channel_id,
                                    crate::utils::format_thinking(&name, &args),
                                )
                                .await;
                            }
                        }
                        VizierResponse {
                            content: VizierResponseContent::Thinking(thought),
                            ..
                        } => {
                            if show_thinking {
                                let _ = crate::utils::discord::send_message(
                                    http.clone(),
                                    &discord_channel_id,
                                    format!("> {}", thought),
                                )
                                .await;
                            }
                        }
                        VizierResponse {
                            content: VizierResponseContent::Message { content, stats: _ },
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

                            break;
                        }
                        VizierResponse {
                            content: VizierResponseContent::AudioReply(audio_att, text, _),
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
                                    if let Err(err) =
                                        discord_channel_id.send_files(&http, files, builder).await
                                    {
                                        tracing::error!("Failed to send audio reply: {:?}", err);
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

                            break;
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

                            break;
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

                            break;
                        }
                        _ => {
                            break;
                        }
                    }
                }

                if let Some(typing) = typing_state.take() {
                    typing.stop();
                }
            });
        }
    }
}
