use std::sync::{Arc, OnceLock};

use anyhow::{Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use twilight_gateway::{CloseFrame, Event, EventTypeFlags, Intents, Shard, ShardId, StreamExt as _};
use twilight_http::Client;
use twilight_model::application::command::{Command, CommandType};
use twilight_model::application::interaction::application_command::CommandOptionValue;
use twilight_model::application::interaction::{Interaction, InteractionData};
use twilight_model::channel::Message;
use twilight_model::channel::message::EmojiReactionType;
use twilight_model::gateway::GatewayReaction;
use twilight_model::gateway::payload::incoming::Ready;
use twilight_model::http::interaction::{InteractionResponse, InteractionResponseType};
use twilight_model::id::Id;
use twilight_model::id::marker::{ApplicationMarker, ChannelMarker, MessageMarker, UserMarker};
use twilight_util::builder::InteractionResponseDataBuilder;
use twilight_util::builder::command::{CommandBuilder, StringBuilder};

use crate::channels::VizierChannel;
use crate::channels::reactions::{self, ReactionChange, ReactionKind, ReactionTarget};
use crate::dependencies::VizierDependencies;
use crate::schema::{
    PlatformMessageId, TopicId, VizierAttachment, VizierAttachmentContent, VizierChannelId,
    VizierRequest, VizierRequestContent, VizierResponse, VizierResponseContent, VizierSession,
};
use crate::storage::reaction::{Platform, ReactionStorage, Reactor};
use crate::storage::session::SessionStorage;
use crate::storage::state::StateStorage;
use crate::utils::discord::Typing;
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
        let http = Arc::new(Client::new(self.token.clone()));
        let mut shard = Shard::new(ShardId::ONE, self.token.clone(), Intents::all());
        let handler = Arc::new(Handler {
            agent_id: self.agent_id.clone(),
            deps: self.deps.clone(),
            http,
            bot: OnceLock::new(),
        });

        let events = EventTypeFlags::READY
            | EventTypeFlags::MESSAGE_CREATE
            | EventTypeFlags::INTERACTION_CREATE
            | EventTypeFlags::REACTION_ADD
            | EventTypeFlags::REACTION_REMOVE
            | EventTypeFlags::REACTION_REMOVE_ALL
            | EventTypeFlags::REACTION_REMOVE_EMOJI;
        let shutdown = self.shutdown.1.clone();
        let mut closing = false;

        loop {
            tokio::select! {
                Ok(_) = shutdown.recv_async(), if !closing => {
                    closing = true;
                    shard.close(CloseFrame::NORMAL);
                }
                item = shard.next_event(events) => {
                    let Some(item) = item else {
                        if closing {
                            return Ok(());
                        }
                        return Err(anyhow!("discord gateway closed fatally for agent {}", self.agent_id));
                    };

                    let event = match item {
                        Ok(event) => event,
                        Err(err) => {
                            tracing::warn!("discord gateway error: {:?}", err);
                            continue;
                        }
                    };

                    let handler = handler.clone();
                    match event {
                        Event::GatewayClose(_) if closing => return Ok(()),
                        Event::Ready(ready) => {
                            tokio::spawn(async move { handler.ready(ready).await });
                        }
                        Event::InteractionCreate(interaction) => {
                            tokio::spawn(async move { handler.interaction(interaction.0).await });
                        }
                        Event::MessageCreate(msg) => {
                            tokio::spawn(async move { handler.message(msg.0).await });
                        }
                        Event::ReactionAdd(add) => {
                            tokio::spawn(async move { handler.reaction_add(add.0).await });
                        }
                        Event::ReactionRemove(remove) => {
                            tokio::spawn(async move { handler.reaction_remove(remove.0).await });
                        }
                        Event::ReactionRemoveEmoji(clear) => {
                            tokio::spawn(async move {
                                handler
                                    .reaction_clear(
                                        clear.channel_id,
                                        clear.message_id,
                                        ReactionKind::ClearEmoji(discord_emoji_key(&clear.emoji)),
                                    )
                                    .await
                            });
                        }
                        Event::ReactionRemoveAll(clear) => {
                            tokio::spawn(async move {
                                handler
                                    .reaction_clear(clear.channel_id, clear.message_id, ReactionKind::ClearAll)
                                    .await
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    async fn shutdown(&self) -> Result<()> {
        let _ = self.shutdown.0.send_async(true).await;
        Ok(())
    }
}

/// Identity of the bot account, learned from the gateway `READY` event.
struct BotIdentity {
    user_id: Id<UserMarker>,
    name: String,
}

struct Handler {
    agent_id: String,
    deps: VizierDependencies,
    http: Arc<Client>,
    bot: OnceLock<BotIdentity>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ChannelState {
    active_topic: Option<TopicId>,
    #[serde(default)]
    show_thinking: bool,
    #[serde(default)]
    show_tool_calls: bool,
}

fn slash_commands() -> Vec<Command> {
    let command = |name: &str, description: &str| {
        CommandBuilder::new(name, description, CommandType::ChatInput)
    };

    vec![
        command("ping", "a simple ping").build(),
        command("new", "create fresh new session").build(),
        command("session", "list or select session")
            .option(StringBuilder::new("topic_id", "switch to the topic if not empty"))
            .build(),
        command("abort", "abort current thinking").build(),
        command("checkpoint", "save checkpoint with handover summary").build(),
        command("lobotomy", "save checkpoint without handover (clean break)").build(),
        command("thinking", "toggle showing thinking output").build(),
        command("tool_calls", "toggle showing tool call details").build(),
    ]
}

fn error_kind_label(kind: &crate::schema::ErrorKind) -> &'static str {
    match kind {
        crate::schema::ErrorKind::Completion => "Completion Error",
        crate::schema::ErrorKind::ToolTimeout => "Tool Timeout",
        crate::schema::ErrorKind::PromptTimeout => "Prompt Timeout",
    }
}

impl Handler {
    fn state_key(&self, channel: &VizierChannelId) -> String {
        format!("{}__{}", self.agent_id, channel.to_slug())
    }

    async fn load_state(&self, channel: &VizierChannelId) -> ChannelState {
        match self.deps.storage.get_state(self.state_key(channel)).await {
            Ok(Some(value)) => serde_json::from_value(value).unwrap_or_default(),
            _ => ChannelState::default(),
        }
    }

    async fn save_state(&self, channel: &VizierChannelId, state: &ChannelState) {
        match serde_json::to_value(state) {
            Ok(value) => {
                if let Err(err) = self.deps.storage.save_state(self.state_key(channel), value).await {
                    tracing::error!("failed to save discord channel state: {}", err);
                }
            }
            Err(err) => tracing::error!("failed to serialize discord channel state: {}", err),
        }
    }

    async fn ready(&self, ready: Ready) {
        let _ = self.bot.set(BotIdentity {
            user_id: ready.user.id,
            name: ready.user.name.clone(),
        });

        let commands = slash_commands();
        if let Err(err) = self
            .http
            .interaction(ready.application.id)
            .set_global_commands(&commands)
            .await
        {
            tracing::error!("failed to register discord slash commands: {:?}", err);
        }
    }

    async fn respond(&self, interaction: &Interaction, content: impl Into<String>) {
        let response = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(InteractionResponseDataBuilder::new().content(content).build()),
        };

        if let Err(err) = self
            .http
            .interaction(interaction.application_id)
            .create_response(interaction.id, &interaction.token, &response)
            .await
        {
            tracing::error!("failed to respond to discord interaction: {:?}", err);
        }
    }

    async fn interaction(&self, interaction: Interaction) {
        let Some(InteractionData::ApplicationCommand(data)) = &interaction.data else {
            return;
        };
        let Some(channel_id) = interaction.channel.as_ref().map(|channel| channel.id) else {
            return;
        };
        let channel = VizierChannelId::DiscordChanel(channel_id.get());
        let agent_id = self.agent_id.clone();

        match data.name.as_str() {
            "ping" => self.respond(&interaction, "Pong!").await,

            "new" => {
                let topic_id = nanoid::nanoid!(10);
                self.save_state(
                    &channel,
                    &ChannelState {
                        active_topic: Some(topic_id.clone()),
                        ..Default::default()
                    },
                )
                .await;

                self.respond(&interaction, format!("switch to new session: **{}**", topic_id))
                    .await;
            }

            "session" => {
                let raw_topic_id = data.options.iter().find_map(|opt| match &opt.value {
                    CommandOptionValue::String(value) if opt.name == "topic_id" => {
                        Some(value.clone())
                    }
                    _ => None,
                });

                if let Some(raw_topic_id) = raw_topic_id {
                    let topic_id: Option<TopicId> = if raw_topic_id == "DEFAULT" {
                        None
                    } else {
                        Some(raw_topic_id.clone())
                    };

                    if let Ok(Some(_)) = self
                        .deps
                        .storage
                        .get_session_detail_by_topic(
                            agent_id.clone(),
                            channel.clone(),
                            topic_id.clone(),
                        )
                        .await
                    {
                        let mut state = self.load_state(&channel).await;
                        state.active_topic = topic_id;
                        self.save_state(&channel, &state).await;

                        self.respond(
                            &interaction,
                            format!("switch to session: **{}**", raw_topic_id),
                        )
                        .await;
                    } else {
                        self.respond(&interaction, "topic not found").await;
                    }
                } else if let Ok(sessions) = self
                    .deps
                    .storage
                    .get_session_list(agent_id.clone(), Some(channel))
                    .await
                {
                    let output = sessions
                        .iter()
                        .map(|session| {
                            format!(
                                "topic_id: {}\ntitle: {}",
                                session.topic.clone().unwrap_or("DEFAULT".into()),
                                session.title
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");

                    self.respond(&interaction, output).await;
                }
            }

            "help" => {
                self.respond(
                    &interaction,
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
                )
                .await;
            }

            "abort" => {
                let topic_id = self.load_state(&channel).await.active_topic;
                let session = VizierSession(agent_id.clone(), channel, topic_id);
                let _ = self
                    .deps
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
                            scheduled_task: None,
                            background_depth: 0,
                        },
                        None,
                    )
                    .await;

                self.respond(&interaction, "aborting...").await;
            }

            "checkpoint" => {
                self.checkpoint(&interaction, channel, "checkpoint", "creating checkpoint...")
                    .await;
            }

            "lobotomy" => {
                self.checkpoint(&interaction, channel, "lobotomy", "performing lobotomy...")
                    .await;
            }

            "thinking" => {
                let mut state = self.load_state(&channel).await;
                state.show_thinking = !state.show_thinking;
                self.save_state(&channel, &state).await;

                let status = if state.show_thinking { "ON" } else { "OFF" };
                self.respond(&interaction, format!("thinking output: **{}**", status))
                    .await;
            }

            "tool_calls" => {
                let mut state = self.load_state(&channel).await;
                state.show_tool_calls = !state.show_tool_calls;
                self.save_state(&channel, &state).await;

                let status = if state.show_tool_calls { "ON" } else { "OFF" };
                self.respond(&interaction, format!("tool call details: **{}**", status))
                    .await;
            }

            _ => {}
        }
    }

    /// Shared flow for `/checkpoint` and `/lobotomy`: send the command to the agent,
    /// acknowledge immediately, then post a followup once the agent reports back.
    async fn checkpoint(
        &self,
        interaction: &Interaction,
        channel: VizierChannelId,
        command: &str,
        pending_message: &str,
    ) {
        let agent_id = self.agent_id.clone();
        let topic_id = self.load_state(&channel).await.active_topic;
        let session = VizierSession(agent_id.clone(), channel, topic_id);
        let (response_tx, response_rx) = flume::unbounded();
        let _ = self
            .deps
            .transport
            .send_request(
                session,
                VizierRequest {
                    timestamp: Utc::now(),
                    user: agent_id,
                    content: VizierRequestContent::Command(command.to_string()),
                    platform_message_id: None,
                    metadata: serde_json::json!({}),
                    attachments: vec![],
                    expect_audio_reply: None,
                    scheduled_task: None,
                    background_depth: 0,
                },
                Some(response_tx),
            )
            .await;

        self.respond(interaction, pending_message).await;

        let http = self.http.clone();
        let application_id: Id<ApplicationMarker> = interaction.application_id;
        let token = interaction.token.clone();
        tokio::spawn(async move {
            while let Ok(response) = response_rx.recv_async().await {
                let content = match response.content {
                    VizierResponseContent::Checkpoint { handover: Some(_) } => {
                        "✅ checkpoint saved".to_string()
                    }
                    VizierResponseContent::Checkpoint { handover: None } => {
                        "✅ lobotomy performed".to_string()
                    }
                    VizierResponseContent::Error { kind, message } => {
                        format!("**{}**: {}", error_kind_label(&kind), message)
                    }
                    _ => continue,
                };

                if let Err(err) = http
                    .interaction(application_id)
                    .create_followup(&token)
                    .content(&content)
                    .await
                {
                    tracing::error!("failed to send discord followup: {:?}", err);
                }
                break;
            }
        });
    }

    /// A person reacted to a message. Only the agent's own linked replies are recorded, and
    /// never a bot's reaction, the agent's own included (FR-006).
    async fn reaction_add(&self, reaction: GatewayReaction) {
        if self.is_bot(&reaction) {
            return;
        }
        let name = match &reaction.member {
            Some(member) => Some(
                member
                    .nick
                    .clone()
                    .or_else(|| member.user.global_name.clone())
                    .unwrap_or_else(|| member.user.name.clone()),
            ),
            // A DM carries no member; one lookup names the person, and a failure leaves the id.
            None => match self.http.user(reaction.user_id).await {
                Ok(response) => response
                    .model()
                    .await
                    .ok()
                    .map(|user| user.global_name.unwrap_or(user.name)),
                Err(err) => {
                    tracing::debug!("could not resolve discord user {}: {:?}", reaction.user_id, err);
                    None
                }
            },
        };
        let kind = ReactionKind::Add(discord_emoji_key(&reaction.emoji));
        self.apply_reaction(reaction.channel_id, reaction.message_id, reaction.user_id.to_string(), name, kind)
            .await;
    }

    async fn reaction_remove(&self, reaction: GatewayReaction) {
        if self.is_bot(&reaction) {
            return;
        }
        let kind = ReactionKind::Remove(discord_emoji_key(&reaction.emoji));
        self.apply_reaction(reaction.channel_id, reaction.message_id, reaction.user_id.to_string(), None, kind)
            .await;
    }

    /// A moderator cleared one emoji, or every reaction, on a message.
    async fn reaction_clear(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        kind: ReactionKind,
    ) {
        self.apply_reaction(channel_id, message_id, String::new(), None, kind)
            .await;
    }

    fn is_bot(&self, reaction: &GatewayReaction) -> bool {
        self.bot.get().is_some_and(|bot| bot.user_id == reaction.user_id)
            || reaction.member.as_ref().is_some_and(|member| member.user.bot)
    }

    async fn apply_reaction(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        reactor_id: String,
        reactor_name: Option<String>,
        kind: ReactionKind,
    ) {
        let change = ReactionChange {
            target: ReactionTarget::Platform {
                agent_id: self.agent_id.clone(),
                platform: Platform::Discord,
                chat_id: channel_id.to_string(),
                message_id: message_id.to_string(),
            },
            reactor: Reactor {
                id: reactor_id,
                name: reactor_name,
            },
            kind,
        };
        if let Err(err) = reactions::apply(&self.deps.storage, &self.deps.transport, change).await {
            tracing::warn!("failed to record discord reaction on {}: {:?}", message_id, err);
        }
    }

    async fn message(&self, msg: Message) {
        let Some(bot) = self.bot.get() else {
            return;
        };
        if msg.author.id == bot.user_id {
            return;
        }

        let channel = VizierChannelId::DiscordChanel(msg.channel_id.get());
        let ChannelState {
            active_topic: topic_id,
            show_thinking,
            show_tool_calls,
        } = self.load_state(&channel).await;

        let is_dm = msg.guild_id.is_none();
        let is_mention = msg.mentions.iter().any(|mention| mention.id == bot.user_id);

        let mut attachments = vec![];
        for attachment in &msg.attachments {
            let bytes_result = async {
                let resp = reqwest::get(&attachment.url).await?;
                resp.bytes().await
            }
            .await;
            if let Ok(bytes) = bytes_result {
                if let Ok(file_record) = self
                    .deps
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

        let transport = self.deps.transport.clone();
        let file_manager = self.deps.file_manager.clone();
        let http = self.http.clone();
        let storage = self.deps.storage.clone();
        let agent_id = self.agent_id.clone();

        let replied_to = msg
            .referenced_message
            .as_ref()
            .map(|message| message.id.to_string());

        let metadata = json!({
            "sent_at": Utc::now().to_string(),
            "is_reply_message": replied_to.is_some(),
            "replied_message_id": replied_to,
            "message_id": msg.id.to_string(),
            "discord_channel_id": msg.channel_id.to_string(),
            "is_dm": is_dm,
        });

        let session = VizierSession(self.agent_id.clone(), channel, topic_id);

        let mentions: Vec<(Id<UserMarker>, &str)> = msg
            .mentions
            .iter()
            .map(|mention| {
                let name = mention
                    .member
                    .as_ref()
                    .and_then(|member| member.nick.as_deref())
                    .unwrap_or(&mention.name);
                (mention.id, name)
            })
            .collect();
        let content = render_mentions(&msg.content, &mentions, bot);

        let request_content = if !is_mention && !is_dm {
            VizierRequestContent::SilentRead(content)
        } else {
            VizierRequestContent::Chat(content)
        };

        let author_name = msg
            .author
            .global_name
            .as_deref()
            .unwrap_or(&msg.author.name);

        let request = VizierRequest {
            timestamp: chrono::Utc::now(),
            user: format!("@{} (DiscordId: {})", author_name, msg.author.id),
            content: request_content,
            platform_message_id: Some(PlatformMessageId::Discord(msg.id.get())),
            metadata,
            attachments,
            ..Default::default()
        };

        let discord_channel_id: Id<ChannelMarker> = msg.channel_id;

        tokio::spawn(async move {
            let (response_tx, response_rx) = flume::unbounded();

            if let Err(err) = transport
                .send_request(session.clone(), request, Some(response_tx))
                .await
            {
                tracing::error!("{}", err);
                return;
            }

            // Dropping the handle stops the typing indicator.
            let mut typing: Option<Typing> = None;

            while let Ok(response) = response_rx.recv_async().await {
                match response {
                    VizierResponse {
                        content: VizierResponseContent::ThinkingStart,
                        ..
                    } => {
                        typing = Some(Typing::start(http.clone(), discord_channel_id));
                    }
                    VizierResponse {
                        content: VizierResponseContent::ToolChoice { name, args },
                        ..
                    } => {
                        if show_tool_calls {
                            let _ = crate::utils::discord::send_message(
                                http.clone(),
                                discord_channel_id,
                                crate::utils::format_thinking(&name, &args),
                            )
                            .await;
                            // Discord clears a bot's typing indicator when it posts.
                            if typing.is_some() {
                                typing = Some(Typing::start(http.clone(), discord_channel_id));
                            }
                        }
                    }
                    VizierResponse {
                        content: VizierResponseContent::Thinking(thought),
                        ..
                    } => {
                        if show_thinking {
                            let _ = crate::utils::discord::send_message(
                                http.clone(),
                                discord_channel_id,
                                format!("> {}", thought),
                            )
                            .await;
                            if typing.is_some() {
                                typing = Some(Typing::start(http.clone(), discord_channel_id));
                            }
                        }
                    }
                    VizierResponse {
                        content: VizierResponseContent::Message { content, stats: _ },
                        attachments,
                        history_uid,
                        ..
                    } => {
                        typing = None;
                        let content = remove_think_tags(&content);
                        let mut posted = crate::utils::discord::send_message(
                            http.clone(),
                            discord_channel_id,
                            content,
                        )
                        .await
                        .unwrap_or_default();

                        for attachment in &attachments {
                            match file_manager.resolve(attachment).await {
                                Ok((filename, bytes)) => {
                                    if let Ok(id) = crate::utils::discord::send_file(
                                        &http,
                                        discord_channel_id,
                                        filename,
                                        bytes,
                                    )
                                    .await
                                    {
                                        posted.push(id);
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

                        link_reply(&storage, &agent_id, discord_channel_id, &posted, history_uid).await;
                        break;
                    }
                    VizierResponse {
                        content: VizierResponseContent::AudioReply(audio_att, text, _),
                        history_uid,
                        ..
                    } => {
                        typing = None;
                        let mut posted = vec![];
                        if let Some(content) = text {
                            let content = remove_think_tags(&content);
                            posted = crate::utils::discord::send_message(
                                http.clone(),
                                discord_channel_id,
                                content,
                            )
                            .await
                            .unwrap_or_default();
                        }
                        match file_manager.resolve(&audio_att).await {
                            Ok((filename, bytes)) => {
                                if let Ok(id) = crate::utils::discord::send_file(
                                    &http,
                                    discord_channel_id,
                                    filename,
                                    bytes,
                                )
                                .await
                                {
                                    posted.push(id);
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

                        link_reply(&storage, &agent_id, discord_channel_id, &posted, history_uid).await;
                        break;
                    }
                    VizierResponse {
                        content: VizierResponseContent::Abort,
                        ..
                    } => {
                        typing = None;
                        let _ = crate::utils::discord::send_message(
                            http.clone(),
                            discord_channel_id,
                            "thinking aborted".into(),
                        )
                        .await;

                        break;
                    }
                    VizierResponse {
                        content: VizierResponseContent::Error { kind, message },
                        ..
                    } => {
                        typing = None;
                        let _ = crate::utils::discord::send_message(
                            http.clone(),
                            discord_channel_id,
                            format!("**{}**: {}", error_kind_label(&kind), message),
                        )
                        .await;

                        break;
                    }
                    // Mid-turn frames (tool responses, checkpoints) are not the end of the turn.
                    _ => {}
                }
            }

            drop(typing);
        });
    }
}

/// Link every message a reply was posted as to the history entry it renders, so reactions on
/// any chunk or attachment reach that entry. A failure is logged and never fails the send.
async fn link_reply(
    storage: &crate::storage::VizierStorage,
    agent_id: &str,
    channel_id: Id<ChannelMarker>,
    posted: &[Id<MessageMarker>],
    history_uid: Option<String>,
) {
    let Some(uid) = history_uid else { return };
    if posted.is_empty() {
        return;
    }
    let ids: Vec<String> = posted.iter().map(|id| id.to_string()).collect();
    if let Err(err) = storage
        .link_platform_messages(agent_id, Platform::Discord, &channel_id.to_string(), &ids, &uid)
        .await
    {
        tracing::warn!("failed to link discord reply {}: {:?}", uid, err);
    }
}

/// How a Discord emoji is stored: Unicode as itself, a custom emoji in Discord's own wire
/// form, so the agent can read its name.
fn discord_emoji_key(emoji: &EmojiReactionType) -> String {
    match emoji {
        EmojiReactionType::Unicode { name } => name.clone(),
        EmojiReactionType::Custom { animated, id, name } => format!(
            "<{}:{}:{}>",
            if *animated { "a" } else { "" },
            name.as_deref().unwrap_or("unknown_emoji"),
            id
        ),
    }
}

/// Rewrite raw user mentions (`<@id>`, legacy `<@!id>`) into a form the agent can read: the
/// bot's own as `@name (you)`, anyone else's the way message authors are rendered. Mentions are
/// rewritten rather than removed so the sentence keeps its shape and the agent can tell who is
/// being addressed or talked about.
fn render_mentions(content: &str, mentions: &[(Id<UserMarker>, &str)], bot: &BotIdentity) -> String {
    let mut content = content.to_string();
    let bot_mention = (bot.user_id, bot.name.as_str());
    for (id, name) in mentions.iter().chain(std::iter::once(&bot_mention)) {
        let rendered = if *id == bot.user_id {
            format!("@{} (you)", bot.name)
        } else {
            format!("@{} (DiscordId: {})", name, id)
        };
        content = content
            .replace(&format!("<@{}>", id), &rendered)
            .replace(&format!("<@!{}>", id), &rendered);
    }
    content.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_keys_keep_unicode_and_name_custom_emoji() {
        assert_eq!(
            discord_emoji_key(&EmojiReactionType::Unicode { name: "👍".into() }),
            "👍"
        );
        assert_eq!(
            discord_emoji_key(&EmojiReactionType::Custom {
                animated: false,
                id: Id::new(42),
                name: Some("party".into()),
            }),
            "<:party:42>"
        );
        assert_eq!(
            discord_emoji_key(&EmojiReactionType::Custom {
                animated: true,
                id: Id::new(7),
                name: None,
            }),
            "<a:unknown_emoji:7>"
        );
    }

    #[test]
    fn mentions_are_rendered_not_stripped() {
        let bot = BotIdentity {
            user_id: Id::new(1),
            name: "vizier".into(),
        };
        let mentions = [(Id::new(1), "vizier"), (Id::new(2), "alice")];
        assert_eq!(
            render_mentions("<@1> what does <@!2> think?", &mentions, &bot),
            "@vizier (you) what does @alice (DiscordId: 2) think?"
        );
        // An unlisted mention of the bot (e.g. a SilentRead) is still recognised.
        assert_eq!(render_mentions("ask <@1>", &[], &bot), "ask @vizier (you)");
    }
}
