use std::sync::{Arc, OnceLock};

use anyhow::{Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use twilight_gateway::{
    CloseFrame, Event, EventTypeFlags, Intents, Shard, ShardId, StreamExt as _,
};
use twilight_http::Client;
use twilight_model::application::command::{
    Command, CommandOptionChoice, CommandOptionChoiceValue, CommandType,
};
use twilight_model::application::interaction::application_command::CommandOptionValue;
use twilight_model::application::interaction::{Interaction, InteractionData, InteractionType};
use twilight_model::channel::Message;
use twilight_model::channel::message::EmojiReactionType;
use twilight_model::gateway::CloseCode;
use twilight_model::gateway::GatewayReaction;
use twilight_model::gateway::payload::incoming::Ready;
use twilight_model::http::interaction::{InteractionResponse, InteractionResponseType};
use twilight_model::id::Id;
use twilight_model::id::marker::{ApplicationMarker, ChannelMarker, MessageMarker, UserMarker};
use twilight_util::builder::InteractionResponseDataBuilder;
use twilight_util::builder::command::{CommandBuilder, StringBuilder};

use crate::channels::commands::{self, CommandEntry, CommandOption, CommandSync, PlatformRules};
use crate::channels::reactions::{self, ReactionChange, ReactionKind, ReactionTarget};
use crate::channels::woken;
use crate::channels::{FatalChannelError, VizierChannel};
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

/// Why the gateway stream ended. Close codes Discord says not to reconnect after (a bad
/// token, intents the bot isn't allowed) need a config change, so retrying them is pointless.
fn gateway_closed(code: Option<u16>) -> anyhow::Error {
    match code.and_then(|code| CloseCode::try_from(code).ok()) {
        Some(code) if !code.can_reconnect() => FatalChannelError(format!(
            "discord refused the connection ({code:?}, close code {}). Check the bot token \
             and that the bot is allowed the intents it asks for (message content, members, \
             presence) in the Discord developer portal",
            code as u16
        ))
        .into(),
        _ => anyhow!("discord gateway closed (close code {code:?})"),
    }
}

impl DiscordChannelReader {
    pub fn new(agent_id: String, token: String, deps: VizierDependencies) -> Self {
        Self {
            deps,
            agent_id,
            token,
            shutdown: flume::bounded(1),
        }
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
            application_id: OnceLock::new(),
            commands: Arc::new(CommandSync::new(
                self.agent_id.clone(),
                &self.deps.config.workspace,
                discord_rules(),
            )),
            clear_confirmations: commands::ClearConfirmations::new(),
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
        // The last close Discord sent, to tell a bad token or intents from a dropped line.
        let mut last_close: Option<u16> = None;

        let _woken = woken::watch(
            &self.deps.transport,
            self.agent_id.clone(),
            |channel| matches!(channel, VizierChannelId::DiscordChanel(_)),
            {
                let handler = handler.clone();
                move |session, frames| {
                    let handler = handler.clone();
                    async move { handler.render_woken(session, frames).await }
                }
            },
        );

        // Keeps the bot's slash commands in step with the agent's skills (spec 021). It sends
        // nothing until `ready()` has learned the application id.
        let _commands = commands::watch(handler.commands.clone(), {
            let handler = handler.clone();
            move |entries| {
                let handler = handler.clone();
                async move { handler.register_commands(entries).await }
            }
        });

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
                        return Err(gateway_closed(last_close));
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
                        Event::GatewayClose(frame) => {
                            last_close = frame.map(|frame| frame.code);
                            tracing::warn!(agent_id = %self.agent_id, code = ?last_close, "discord gateway closed the connection");
                        }
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
    application_id: OnceLock<Id<ApplicationMarker>>,
    commands: Arc<CommandSync>,
    /// Pending `/clear-files` two-step confirmations (`specs/035-add-artifacts`).
    clear_confirmations: commands::ClearConfirmations,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ChannelState {
    active_topic: Option<TopicId>,
    #[serde(default)]
    show_thinking: bool,
    #[serde(default)]
    show_tool_calls: bool,
}

fn builtin_commands() -> Vec<CommandEntry> {
    vec![
        CommandEntry::new("ping", "a simple ping"),
        CommandEntry::new("help", "what I can do"),
        CommandEntry::new("new", "create fresh new session"),
        CommandEntry::new("session", "list or select session").option(CommandOption::text(
            "topic_id",
            "switch to the topic if not empty",
        )),
        CommandEntry::new("abort", "abort current thinking"),
        CommandEntry::new("checkpoint", "save checkpoint with handover summary"),
        CommandEntry::new("lobotomy", "save checkpoint without handover (clean break)"),
        CommandEntry::new("thinking", "toggle showing thinking output"),
        CommandEntry::new("tool_calls", "toggle showing tool call details"),
        CommandEntry::new("clear-files", "clear this conversation's session files")
            .option(CommandOption::text("confirm", "Type confirm to clear")),
    ]
}

/// Global commands: names of 1–32 `a-z0-9_-`, descriptions up to 100 characters, at most
/// 100 commands. Skills are lower-cased; one that still does not fit is run with `/skill`.
fn discord_rules() -> PlatformRules {
    PlatformRules {
        name_ok: |name| {
            commands::name_ok(name, |c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
            })
        },
        convert: str::to_lowercase,
        max_commands: 100,
        max_description: 100,
        builtins: builtin_commands(),
    }
}

fn slash_command(entry: &CommandEntry) -> Command {
    let mut command = CommandBuilder::new(&entry.name, &entry.description, CommandType::ChatInput);
    for option in &entry.options {
        command = command.option(
            StringBuilder::new(&option.name, &option.description)
                .required(option.required)
                .autocomplete(option.autocomplete),
        );
    }
    command.build()
}

/// The string value of a command option, `""` when absent.
fn option_text<'a>(
    options: &'a [twilight_model::application::interaction::application_command::CommandDataOption],
    name: &str,
) -> &'a str {
    options
        .iter()
        .find_map(|opt| match &opt.value {
            CommandOptionValue::String(value) if opt.name == name => Some(value.as_str()),
            CommandOptionValue::Focused(value, _) if opt.name == name => Some(value.as_str()),
            _ => None,
        })
        .unwrap_or("")
}

/// The chat text a skill command stands for, which the agent resolves like typed text.
fn skill_command_text(
    command: &str,
    options: &[twilight_model::application::interaction::application_command::CommandDataOption],
) -> String {
    let input = option_text(options, "input").trim();
    let head = match command {
        "skill" => format!("/skill {}", option_text(options, "name").trim()),
        other => format!("/{other}"),
    };
    if input.is_empty() {
        head
    } else {
        format!("{head} {input}")
    }
}

fn error_kind_label(kind: &crate::schema::ErrorKind) -> &'static str {
    kind.label()
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
                if let Err(err) = self
                    .deps
                    .storage
                    .save_state(self.state_key(channel), value)
                    .await
                {
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

        let _ = self.application_id.set(ready.application.id);
        // `Ready` repeats on every full reconnect; the sync sends only when the list changed.
        self.commands
            .sync(|entries| self.register_commands(entries))
            .await;
    }

    /// Replace the bot's global slash commands. Refused until `ready()` knows the
    /// application id, so the first tick of the re-sync before then is not recorded as sent.
    async fn register_commands(&self, entries: Vec<CommandEntry>) -> Result<()> {
        let Some(application_id) = self.application_id.get() else {
            return Err(anyhow!("not connected yet"));
        };
        let commands: Vec<Command> = entries.iter().map(slash_command).collect();
        self.http
            .interaction(*application_id)
            .set_global_commands(&commands)
            .await
            .map_err(|err| anyhow!("{err:?}"))?;
        Ok(())
    }

    async fn respond(&self, interaction: &Interaction, content: impl Into<String>) {
        let response = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(
                InteractionResponseDataBuilder::new()
                    .content(content)
                    .build(),
            ),
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
        if interaction.kind == InteractionType::ApplicationCommandAutocomplete {
            self.autocomplete(&interaction, &data.options).await;
            return;
        }
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

                self.respond(
                    &interaction,
                    format!("switch to new session: **{}**", topic_id),
                )
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

**Skills:**
• `/skills` — List the skills you can run
• `/<skill> [input]` — Run a skill (each one is in the command list)
• `/skill <name> [input]` — Run any skill by name
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
                            trace: Default::default(),
                            client_tools: vec![],
                            client_categories: vec![],
                            client: None,
                            timestamp: Utc::now(),
                            user: agent_id.clone(),
                            content: VizierRequestContent::Command("abort".to_string()),
                            platform_message_id: None,
                            metadata: serde_json::json!({}),
                            attachments: vec![],
                            expect_audio_reply: None,
                            scheduled_task: None,
                            background_depth: 0,
                            ..Default::default()
                        },
                        None,
                    )
                    .await;

                self.respond(&interaction, "aborting...").await;
            }

            "checkpoint" => {
                self.checkpoint(
                    &interaction,
                    channel,
                    "checkpoint",
                    "creating checkpoint...",
                )
                .await;
            }

            "lobotomy" => {
                self.checkpoint(&interaction, channel, "lobotomy", "performing lobotomy...")
                    .await;
            }

            "clear-files" => {
                let topic_id = self.load_state(&channel).await.active_topic;
                let session = VizierSession(agent_id.clone(), channel.clone(), topic_id);
                let confirm = data.options.iter().find_map(|opt| match &opt.value {
                    CommandOptionValue::String(value)
                        if opt.name == "confirm" && value == "confirm" =>
                    {
                        Some(())
                    }
                    _ => None,
                });
                if confirm.is_some() {
                    if !self.clear_confirmations.take(&session) {
                        self.respond(
                            &interaction,
                            "No clear is waiting; send /clear-files first.",
                        )
                        .await;
                        return;
                    }
                    if self.deps.session_files.has_active_turn(&session).await {
                        self.respond(
                            &interaction,
                            "a turn is running or queued in this conversation; wait for it to finish",
                        )
                        .await;
                        return;
                    }
                    match self.deps.session_files.clear(&session).await {
                        Ok(result) => {
                            self.respond(
                                &interaction,
                                format!(
                                    "Cleared {} session files. Artifacts are untouched.",
                                    result.removed
                                ),
                            )
                            .await
                        }
                        Err(e) => self.respond(&interaction, e.to_string()).await,
                    }
                    return;
                }
                if self.deps.session_files.has_active_turn(&session).await {
                    self.respond(
                        &interaction,
                        "a turn is running or queued in this conversation; wait for it to finish",
                    )
                    .await;
                    return;
                }
                let count = self.deps.session_files.count(&session).await.unwrap_or(0);
                if count == 0 {
                    self.respond(
                        &interaction,
                        "There are no session files to clear in this conversation.",
                    )
                    .await;
                    return;
                }
                self.clear_confirmations.request(&session);
                self.respond(
                    &interaction,
                    format!(
                        "This will remove {count} session files from this conversation. Artifacts are unaffected. Older messages or memories that reference them will no longer open. Send /clear-files confirm within 60 seconds to continue."
                    ),
                )
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

            // Any other command is `/skill`, `/skills` or one skill's own command.
            command => {
                let text = skill_command_text(command, &data.options);
                self.run_skill_command(&interaction, channel, text).await;
            }
        }
    }

    /// Suggest skills for `/skill name:` as the person types (Discord shows up to 25).
    async fn autocomplete(
        &self,
        interaction: &Interaction,
        options: &[twilight_model::application::interaction::application_command::CommandDataOption],
    ) {
        let typed = option_text(options, "name").to_lowercase();
        let skills = self.commands.skills().unwrap_or_default();
        let (mut starts, contains): (Vec<_>, Vec<_>) = skills
            .iter()
            .filter(|skill| skill.name.to_lowercase().contains(&typed))
            .partition(|skill| skill.name.to_lowercase().starts_with(&typed));
        starts.extend(contains);
        let choices = starts
            .into_iter()
            .take(25)
            .filter(|skill| skill.name.chars().count() <= 100)
            .map(|skill| CommandOptionChoice {
                name: skill.name.clone(),
                name_localizations: None,
                value: CommandOptionChoiceValue::String(skill.name.clone()),
            });
        let response = InteractionResponse {
            kind: InteractionResponseType::ApplicationCommandAutocompleteResult,
            data: Some(
                InteractionResponseDataBuilder::new()
                    .choices(choices)
                    .build(),
            ),
        };
        if let Err(err) = self
            .http
            .interaction(interaction.application_id)
            .create_response(interaction.id, &interaction.token, &response)
            .await
        {
            tracing::debug!("failed to answer discord autocomplete: {:?}", err);
        }
    }

    /// A skill picked from the command list: sent as the chat text it stands for, so the
    /// agent resolves it exactly as it would typed text, and the reply posted like any turn.
    async fn run_skill_command(
        &self,
        interaction: &Interaction,
        channel: VizierChannelId,
        text: String,
    ) {
        let deferred = InteractionResponse {
            kind: InteractionResponseType::DeferredChannelMessageWithSource,
            data: None,
        };
        if let Err(err) = self
            .http
            .interaction(interaction.application_id)
            .create_response(interaction.id, &interaction.token, &deferred)
            .await
        {
            tracing::error!("failed to respond to discord interaction: {:?}", err);
            return;
        }
        let Some(channel_id) = interaction.channel.as_ref().map(|channel| channel.id) else {
            return;
        };

        let ChannelState {
            active_topic,
            show_thinking,
            show_tool_calls,
        } = self.load_state(&channel).await;
        let session = VizierSession(self.agent_id.clone(), channel, active_topic);
        let user = match interaction.author() {
            Some(author) => format!(
                "@{} (DiscordId: {})",
                author.global_name.as_deref().unwrap_or(&author.name),
                author.id
            ),
            None => "unknown".to_string(),
        };
        let request = VizierRequest {
            timestamp: Utc::now(),
            user,
            content: VizierRequestContent::Chat(text.clone()),
            metadata: json!({
                "sent_at": Utc::now().to_string(),
                "is_reply_message": false,
                "discord_channel_id": channel_id.to_string(),
                "is_dm": interaction.guild_id.is_none(),
                "via": "slash_command",
            }),
            ..Default::default()
        };
        let renderer = TurnRenderer {
            http: self.http.clone(),
            file_manager: self.deps.file_manager.clone(),
            storage: self.deps.storage.clone(),
            agent_id: self.agent_id.clone(),
            channel_id,
            show_thinking,
            show_tool_calls,
        };

        let (response_tx, response_rx) = flume::unbounded();
        if let Err(err) = self
            .deps
            .transport
            .send_request(session, request, Some(response_tx))
            .await
        {
            tracing::error!("{}", err);
        } else {
            renderer.render(response_rx).await;
        }

        // Replaces Discord's "thinking…" with what was run, once the reply is posted.
        let echo = format!("`{text}`");
        if let Err(err) = self
            .http
            .interaction(interaction.application_id)
            .update_response(&interaction.token)
            .content(Some(&echo))
            .await
        {
            tracing::debug!("failed to finish discord interaction: {:?}", err);
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
                    trace: Default::default(),
                    client_tools: vec![],
                    client_categories: vec![],
                    client: None,
                    timestamp: Utc::now(),
                    user: agent_id,
                    content: VizierRequestContent::Command(command.to_string()),
                    platform_message_id: None,
                    metadata: serde_json::json!({}),
                    attachments: vec![],
                    expect_audio_reply: None,
                    scheduled_task: None,
                    background_depth: 0,
                    ..Default::default()
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
                    tracing::debug!(
                        "could not resolve discord user {}: {:?}",
                        reaction.user_id,
                        err
                    );
                    None
                }
            },
        };
        let kind = ReactionKind::Add(discord_emoji_key(&reaction.emoji));
        self.apply_reaction(
            reaction.channel_id,
            reaction.message_id,
            reaction.user_id.to_string(),
            name,
            kind,
        )
        .await;
    }

    async fn reaction_remove(&self, reaction: GatewayReaction) {
        if self.is_bot(&reaction) {
            return;
        }
        let kind = ReactionKind::Remove(discord_emoji_key(&reaction.emoji));
        self.apply_reaction(
            reaction.channel_id,
            reaction.message_id,
            reaction.user_id.to_string(),
            None,
            kind,
        )
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
        self.bot
            .get()
            .is_some_and(|bot| bot.user_id == reaction.user_id)
            || reaction
                .member
                .as_ref()
                .is_some_and(|member| member.user.bot)
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
            tracing::warn!(
                "failed to record discord reaction on {}: {:?}",
                message_id,
                err
            );
        }
    }

    /// Post a turn a background report woke into the Discord channel the work came from.
    async fn render_woken(&self, session: VizierSession, frames: flume::Receiver<VizierResponse>) {
        let VizierChannelId::DiscordChanel(channel_id) = session.1 else {
            return;
        };
        let Some(channel_id) = Id::<ChannelMarker>::new_checked(channel_id) else {
            return;
        };
        let state = self.load_state(&session.1).await;
        TurnRenderer {
            http: self.http.clone(),
            file_manager: self.deps.file_manager.clone(),
            storage: self.deps.storage.clone(),
            agent_id: self.agent_id.clone(),
            channel_id,
            show_thinking: state.show_thinking,
            show_tool_calls: state.show_tool_calls,
        }
        .render(frames)
        .await;
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

        // `/abort` typed as text rather than picked as the slash command. It must reach the
        // agent as a command: as chat it would queue behind the very turn it means to stop.
        if (is_dm || is_mention) && is_text_abort(&msg.content, bot.user_id) {
            let session = VizierSession(self.agent_id.clone(), channel, topic_id);
            let renderer = TurnRenderer {
                http: self.http.clone(),
                file_manager: self.deps.file_manager.clone(),
                storage: self.deps.storage.clone(),
                agent_id: self.agent_id.clone(),
                channel_id: msg.channel_id,
                show_thinking,
                show_tool_calls,
            };
            let (response_tx, response_rx) = flume::unbounded();
            let request = VizierRequest {
                timestamp: Utc::now(),
                user: self.agent_id.clone(),
                content: VizierRequestContent::Command("abort".to_string()),
                ..Default::default()
            };
            if let Err(err) = self
                .deps
                .transport
                .send_request(session, request, Some(response_tx))
                .await
            {
                tracing::error!("{}", err);
                return;
            }
            // Posts "thinking aborted" if a turn was stopped; nothing if there was none.
            tokio::spawn(renderer.render(response_rx));
            return;
        }

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
        // `@bot /release-notes x`: the agent must see the command first to resolve it.
        let content = render_mentions(&command_text(&msg.content, bot.user_id), &mentions, bot);

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

        let renderer = TurnRenderer {
            http: self.http.clone(),
            file_manager: self.deps.file_manager.clone(),
            storage: self.deps.storage.clone(),
            agent_id: self.agent_id.clone(),
            channel_id: msg.channel_id,
            show_thinking,
            show_tool_calls,
        };

        tokio::spawn(async move {
            let (response_tx, response_rx) = flume::unbounded();

            if let Err(err) = transport
                .send_request(session.clone(), request, Some(response_tx))
                .await
            {
                tracing::error!("{}", err);
                return;
            }

            renderer.render(response_rx).await;
        });
    }
}

/// Posts one turn's frames to a Discord channel: a person's turn, or one a background report
/// woke (`channels::woken`).
struct TurnRenderer {
    http: Arc<Client>,
    file_manager: crate::file_manager::FileManager,
    storage: Arc<crate::storage::VizierStorage>,
    agent_id: String,
    channel_id: Id<ChannelMarker>,
    show_thinking: bool,
    show_tool_calls: bool,
}

impl TurnRenderer {
    async fn render(self, response_rx: flume::Receiver<VizierResponse>) {
        let Self {
            http,
            file_manager,
            storage,
            agent_id,
            channel_id: discord_channel_id,
            show_thinking,
            show_tool_calls,
        } = self;

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
                    resumed,
                    ..
                } => {
                    typing = None;
                    let content =
                        crate::channels::continued_note(resumed, remove_think_tags(&content));
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
                                match crate::utils::discord::send_file(
                                    &http,
                                    discord_channel_id,
                                    filename,
                                    bytes,
                                )
                                .await
                                {
                                    Ok(id) => posted.push(id),
                                    Err(err) => {
                                        tracing::error!(
                                            "Failed to send attachment {:?}: {:?}",
                                            attachment.filename,
                                            err
                                        );
                                        let _ = crate::utils::discord::send_message(
                                            http.clone(),
                                            discord_channel_id,
                                            crate::channels::attachment_failure_notice(
                                                &attachment.filename,
                                            ),
                                        )
                                        .await;
                                    }
                                }
                            }
                            Err(err) => {
                                tracing::error!(
                                    "Failed to resolve attachment {:?}: {:?}",
                                    attachment.filename,
                                    err
                                );
                                let _ = crate::utils::discord::send_message(
                                    http.clone(),
                                    discord_channel_id,
                                    crate::channels::attachment_failure_notice(
                                        &attachment.filename,
                                    ),
                                )
                                .await;
                            }
                        }
                    }

                    link_reply(
                        &storage,
                        &agent_id,
                        discord_channel_id,
                        &posted,
                        history_uid,
                    )
                    .await;
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

                    link_reply(
                        &storage,
                        &agent_id,
                        discord_channel_id,
                        &posted,
                        history_uid,
                    )
                    .await;
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
        .link_platform_messages(
            agent_id,
            Platform::Discord,
            &channel_id.to_string(),
            &ids,
            &uid,
        )
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
/// The message is `/abort`, alone or after mentions of the bot.
fn is_text_abort(content: &str, bot_id: Id<UserMarker>) -> bool {
    let text = content
        .replace(&format!("<@{}>", bot_id), "")
        .replace(&format!("<@!{}>", bot_id), "");
    text.trim().eq_ignore_ascii_case("/abort")
}

/// The message with a leading mention of the bot removed when what follows is a slash
/// command; otherwise unchanged.
fn command_text(content: &str, bot_id: Id<UserMarker>) -> String {
    let trimmed = content.trim_start();
    let rest = trimmed
        .strip_prefix(&format!("<@{}>", bot_id))
        .or_else(|| trimmed.strip_prefix(&format!("<@!{}>", bot_id)));
    match rest {
        Some(rest)
            if crate::skill::invocation::parse(rest) != crate::skill::invocation::Parsed::Plain =>
        {
            rest.trim().to_string()
        }
        _ => content.to_string(),
    }
}

fn render_mentions(
    content: &str,
    mentions: &[(Id<UserMarker>, &str)],
    bot: &BotIdentity,
) -> String {
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
    fn text_abort_is_recognised_with_or_without_a_mention() {
        let bot = Id::new(42);
        assert!(is_text_abort("/abort", bot));
        assert!(is_text_abort("<@42> /abort ", bot));
        assert!(is_text_abort("<@!42>/abort", bot));
        assert!(!is_text_abort("/abort the plan", bot));
        assert!(!is_text_abort("<@7> /abort", bot));
    }

    #[test]
    fn a_leading_mention_is_dropped_before_a_command() {
        let bot = Id::new(42);
        assert_eq!(
            command_text("<@42> /release-notes v1", bot),
            "/release-notes v1"
        );
        assert_eq!(command_text("<@!42>/skills", bot), "/skills");
        assert_eq!(command_text("<@42> hello", bot), "<@42> hello");
        assert_eq!(command_text("<@7> /x", bot), "<@7> /x");
    }

    #[test]
    fn skill_commands_become_the_text_they_stand_for() {
        use twilight_model::application::interaction::application_command::CommandDataOption;
        let option = |name: &str, value: &str| CommandDataOption {
            name: name.into(),
            value: CommandOptionValue::String(value.into()),
        };
        assert_eq!(
            skill_command_text("release-notes", &[option("input", " v1.2 ")]),
            "/release-notes v1.2"
        );
        assert_eq!(skill_command_text("release-notes", &[]), "/release-notes");
        assert_eq!(
            skill_command_text(
                "skill",
                &[option("name", "standup"), option("input", "today")]
            ),
            "/skill standup today"
        );
        assert_eq!(skill_command_text("skills", &[]), "/skills");
    }

    #[test]
    fn emoji_keys_keep_unicode_and_name_custom_emoji() {
        assert_eq!(
            discord_emoji_key(&EmojiReactionType::Unicode {
                name: "👍".into()
            }),
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
