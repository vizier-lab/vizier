use std::sync::atomic::{AtomicI64, Ordering};

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use teloxide::Bot;
use teloxide::prelude::*;
use teloxide::types::{
    AllowedUpdate, BotCommand, ChatAction, InputFile, MaybeAnonymousUser, MessageId,
    MessageReactionUpdated, ReactionType,
};
use teloxide::{ApiError, RequestError};

use crate::channels::commands::{self, CommandEntry, CommandSync, PlatformRules};
use crate::channels::reactions::{self, ReactionChange, ReactionKind, ReactionTarget};
use crate::channels::woken;
use crate::channels::{FatalChannelError, VizierChannel};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::{
    PlatformMessageId, TopicId, VizierAttachment, VizierAttachmentContent, VizierChannelId,
    VizierRequest, VizierRequestContent, VizierResponse, VizierResponseContent, VizierSession,
};
use crate::storage::reaction::{Platform, ReactionStorage, Reactor};
use crate::storage::session::SessionStorage;
use crate::storage::state::StateStorage;
use crate::transport::VizierTransport;
use crate::utils::{get_mime_type, remove_think_tags};

pub struct TelegramChannelReader {
    bot: Bot,
    token: String,
    agent_id: String,
    deps: VizierDependencies,
    /// The bot's command menu, and the agent's skills for unmentioned `/<skill>` in groups.
    commands: std::sync::Arc<CommandSync>,
    /// The next update to ask for. Kept across restarts of `run`, so a restart doesn't ask
    /// Telegram again for updates already handled.
    offset: AtomicI64,
    /// Pending `/clear_files` two-step confirmations (`specs/035-add-artifacts`).
    clear_confirmations: commands::ClearConfirmations,
}

impl TelegramChannelReader {
    pub fn new(agent_id: String, token: String, deps: VizierDependencies) -> Self {
        let bot = Bot::new(token.clone());
        let commands = std::sync::Arc::new(CommandSync::new(
            agent_id.clone(),
            &deps.config.workspace,
            telegram_rules(),
        ));

        Self {
            bot,
            token,
            agent_id,
            deps,
            commands,
            offset: AtomicI64::new(0),
            clear_confirmations: commands::ClearConfirmations::new(),
        }
    }
}

#[async_trait::async_trait]
impl VizierChannel for TelegramChannelReader {
    async fn run(&self) -> Result<()> {
        // The command menu: set now, then kept in step with the skills (spec 021). A failure
        // is only logged; a bad token already ends the run through `get_updates`.
        let _commands = commands::watch(self.commands.clone(), {
            let bot = self.bot.clone();
            move |entries: Vec<CommandEntry>| {
                let bot = bot.clone();
                async move {
                    let menu = entries
                        .into_iter()
                        .map(|entry| BotCommand::new(entry.name, entry.description));
                    bot.set_my_commands(menu).await?;
                    Ok(())
                }
            }
        });

        let _woken = woken::watch(
            &self.deps.transport,
            self.agent_id.clone(),
            |channel| matches!(channel, VizierChannelId::TelegramChannel(_)),
            {
                let bot = self.bot.clone();
                let deps = self.deps.clone();
                move |session, frames| {
                    Self::render_woken(bot.clone(), deps.clone(), session, frames)
                }
            },
        );
        loop {
            let updates = self
                .bot
                .get_updates()
                .offset(self.offset.load(Ordering::SeqCst) as i32)
                // Telegram sends `message_reaction` only when asked for it by name.
                .allowed_updates(vec![
                    AllowedUpdate::Message,
                    AllowedUpdate::EditedMessage,
                    AllowedUpdate::MessageReaction,
                ])
                .timeout(30)
                .await;

            match updates {
                Ok(updates) => {
                    for update in updates {
                        self.offset.store(update.id.0 as i64 + 1, Ordering::SeqCst);
                        if let Err(e) = self.handle_update(update).await {
                            tracing::error!("Error handling update: {:?}", e);
                        }
                    }
                }
                // Asked to slow down: not a failure, so no restart and no backoff growth.
                Err(RequestError::RetryAfter(wait)) => {
                    tokio::time::sleep(wait.duration()).await;
                }
                // Ends this run; the reader's supervisor logs it and retries with backoff
                // (#50), instead of the silent fixed 1s loop this used to be.
                Err(RequestError::Api(ApiError::InvalidToken)) => {
                    return Err(FatalChannelError(
                        "telegram rejected the bot token (revoked, or mistyped)".into(),
                    )
                    .into());
                }
                Err(RequestError::Api(ApiError::TerminatedByOtherGetUpdates)) => {
                    return Err(anyhow::anyhow!(
                        "another process is polling this telegram bot; only one may at a time"
                    ));
                }
                Err(e) => return Err(anyhow::anyhow!("telegram polling failed: {e}")),
            }
        }
    }
}

impl TelegramChannelReader {
    async fn handle_update(&self, update: Update) -> Result<()> {
        let kind = &update.kind;

        match kind {
            teloxide::types::UpdateKind::Message(msg) => {
                self.handle_message(msg.clone()).await?;
            }
            teloxide::types::UpdateKind::EditedMessage(msg) => {
                self.handle_message(msg.clone()).await?;
            }
            teloxide::types::UpdateKind::MessageReaction(update) => {
                self.handle_reaction(update).await;
            }
            _ => return Ok(()),
        }

        Ok(())
    }

    /// A person changed their reactions on a message. Telegram sends the whole old and new
    /// set, so the change is the difference: removals first, so a swap ends on the new emoji.
    /// Only the agent's own linked replies are recorded, and never a bot's reaction (FR-006).
    async fn handle_reaction(&self, update: &MessageReactionUpdated) {
        let reactor = match &update.actor {
            MaybeAnonymousUser::User(user) => {
                if user.is_bot {
                    return;
                }
                let name = match &user.last_name {
                    Some(last) => format!("{} {}", user.first_name, last),
                    None => user.first_name.clone(),
                };
                let name = if name.trim().is_empty() {
                    user.username
                        .as_ref()
                        .map(|username| format!("@{username}"))
                } else {
                    Some(name)
                };
                Reactor {
                    id: user.id.0.to_string(),
                    name,
                }
            }
            // An anonymous admin reacts as the chat itself.
            MaybeAnonymousUser::Chat(chat) => Reactor {
                id: chat.id.0.to_string(),
                name: chat.title().map(str::to_string),
            },
        };

        let (removed, added) = reaction_diff(&update.old_reaction, &update.new_reaction);
        let changes = removed
            .into_iter()
            .map(ReactionKind::Remove)
            .chain(added.into_iter().map(ReactionKind::Add));
        for kind in changes {
            let change = ReactionChange {
                target: ReactionTarget::Platform {
                    agent_id: self.agent_id.clone(),
                    platform: Platform::Telegram,
                    chat_id: update.chat.id.0.to_string(),
                    message_id: update.message_id.0.to_string(),
                },
                reactor: reactor.clone(),
                kind,
            };
            if let Err(err) =
                reactions::apply(&self.deps.storage, &self.deps.transport, change).await
            {
                tracing::warn!(
                    "failed to record telegram reaction on {}: {:?}",
                    update.message_id.0,
                    err
                );
            }
        }
    }

    async fn handle_message(&self, msg: Message) -> Result<()> {
        let chat_id = msg.chat.id;
        let channel = VizierChannelId::TelegramChannel(chat_id.0);

        let is_dm = msg.chat.is_private();

        let key = format!("{}__{}", self.agent_id, channel.to_slug());
        let (topic_id, show_thinking, show_tool_calls) =
            if let Ok(Some(value)) = self.deps.storage.get_state(key.clone()).await {
                if let Ok(state) = serde_json::from_value::<ChannelState>(value) {
                    (
                        state.active_topic,
                        state.show_thinking,
                        state.show_tool_calls,
                    )
                } else {
                    (None, false, false)
                }
            } else {
                (None, false, false)
            };

        let text = msg.text().unwrap_or("").to_string();
        let bot_username = self.bot.get_me().await?.username().to_string();

        let is_mention = text.starts_with(&format!("@{}", bot_username))
            || text.contains(&format!("@{}", bot_username));

        let replied_to = msg.reply_to_message().as_ref().map(|m| m.id.to_string());
        let message_id = msg.id.to_string();
        let user_full_name = msg
            .from
            .as_ref()
            .map(|u| u.username.clone().unwrap_or_default())
            .unwrap_or_else(|| "Unknown".into());

        let metadata = serde_json::json!({
            "sent_at": Utc::now().to_string(),
            "is_reply_message": replied_to.is_some(),
            "replied_message_id": replied_to,
            "message_id": message_id,
            "telegram_chat_id": chat_id.to_string(),
            "is_dm": is_dm,
        });

        let mut attachments = vec![];

        if let Some(photo) = msg.photo() {
            if let Some(photo) = photo.iter().max_by_key(|p| p.width * p.height) {
                let file_id = photo.file.id.clone();
                if let Ok(file) = self.bot.get_file(file_id.clone()).await {
                    let url = format!(
                        "https://api.telegram.org/file/bot{}/{}",
                        self.token, file.path
                    );
                    let bytes = reqwest::get(&url).await?.bytes().await?.to_vec();
                    let file_record = self
                        .deps
                        .transport
                        .send_file_upload(format!("photo_{}.jpg", file_id), bytes)
                        .await
                        .map_err(|e| VizierError(e.to_string()))?;
                    attachments.push(VizierAttachment {
                        filename: format!("photo_{}.jpg", file_id),
                        content: VizierAttachmentContent::Local(file_record.url),
                    });
                }
            }
        }

        if let Some(doc) = msg.document() {
            let file_id = doc.file.id.clone();
            if let Ok(file) = self.bot.get_file(file_id.clone()).await {
                let url = format!(
                    "https://api.telegram.org/file/bot{}/{}",
                    self.token, file.path
                );
                let filename = doc
                    .file_name
                    .clone()
                    .unwrap_or_else(|| format!("document_{}", file_id));
                let bytes = reqwest::get(&url).await?.bytes().await?.to_vec();
                let file_record = self
                    .deps
                    .transport
                    .send_file_upload(filename.clone(), bytes)
                    .await
                    .map_err(|e| VizierError(e.to_string()))?;
                attachments.push(VizierAttachment {
                    filename,
                    content: VizierAttachmentContent::Local(file_record.url),
                });
            }
        }

        let user = format!("@{} (TelegramUser: {})", user_full_name, chat_id.0);

        let transport = self.deps.transport.clone();

        if is_command(&text, "ping", &bot_username) {
            let _ = self.bot.send_message(chat_id, "Pong!").await?;
            return Ok(());
        }

        if is_command(&text, "clear_files", &bot_username) {
            let session = VizierSession(self.agent_id.clone(), channel.clone(), topic_id.clone());
            let confirming = text
                .split_whitespace()
                .nth(1)
                .is_some_and(|word| word.eq_ignore_ascii_case("confirm"));
            if confirming {
                if !self.clear_confirmations.take(&session) {
                    let _ = self
                        .bot
                        .send_message(chat_id, "No clear is waiting; send /clear_files first.")
                        .await;
                    return Ok(());
                }
                if self.deps.session_files.has_active_turn(&session).await {
                    let _ = self
                        .bot
                        .send_message(
                            chat_id,
                            "a turn is running or queued in this conversation; wait for it to finish",
                        )
                        .await;
                    return Ok(());
                }
                match self.deps.session_files.clear(&session).await {
                    Ok(result) => {
                        let _ = self
                            .bot
                            .send_message(
                                chat_id,
                                format!(
                                    "Cleared {} session files. Artifacts are untouched.",
                                    result.removed
                                ),
                            )
                            .await;
                    }
                    Err(e) => {
                        let _ = self.bot.send_message(chat_id, e.to_string()).await;
                    }
                }
                return Ok(());
            }
            if self.deps.session_files.has_active_turn(&session).await {
                let _ = self
                    .bot
                    .send_message(
                        chat_id,
                        "a turn is running or queued in this conversation; wait for it to finish",
                    )
                    .await;
                return Ok(());
            }
            let count = self.deps.session_files.count(&session).await.unwrap_or(0);
            if count == 0 {
                let _ = self
                    .bot
                    .send_message(
                        chat_id,
                        "There are no session files to clear in this conversation.",
                    )
                    .await;
                return Ok(());
            }
            self.clear_confirmations.request(&session);
            let _ = self
                .bot
                .send_message(
                    chat_id,
                    format!(
                        "This will remove {count} session files from this conversation. Artifacts are unaffected. Older messages or memories that reference them will no longer open. Send /clear_files confirm within 60 seconds to continue."
                    ),
                )
                .await;
            return Ok(());
        }

        if is_command(&text, "thinking", &bot_username) {
            let mut state = if let Ok(Some(value)) = self.deps.storage.get_state(key.clone()).await
            {
                serde_json::from_value::<ChannelState>(value).unwrap_or(ChannelState {
                    active_topic: None,
                    show_thinking: false,
                    show_tool_calls: false,
                })
            } else {
                ChannelState {
                    active_topic: topic_id.clone(),
                    show_thinking: false,
                    show_tool_calls: false,
                }
            };
            state.show_thinking = !state.show_thinking;
            let _ = self
                .deps
                .storage
                .save_state(key.clone(), serde_json::to_value(&state).unwrap())
                .await;
            let status = if state.show_thinking { "ON" } else { "OFF" };
            let _ = self
                .bot
                .send_message(chat_id, format!("thinking output: {}", status))
                .await?;
            return Ok(());
        }

        if is_command(&text, "tool_calls", &bot_username) {
            let mut state = if let Ok(Some(value)) = self.deps.storage.get_state(key.clone()).await
            {
                serde_json::from_value::<ChannelState>(value).unwrap_or(ChannelState {
                    active_topic: None,
                    show_thinking: false,
                    show_tool_calls: false,
                })
            } else {
                ChannelState {
                    active_topic: topic_id.clone(),
                    show_thinking: false,
                    show_tool_calls: false,
                }
            };
            state.show_tool_calls = !state.show_tool_calls;
            let _ = self
                .deps
                .storage
                .save_state(key.clone(), serde_json::to_value(&state).unwrap())
                .await;
            let status = if state.show_tool_calls { "ON" } else { "OFF" };
            let _ = self
                .bot
                .send_message(chat_id, format!("tool call details: {}", status))
                .await?;
            return Ok(());
        }

        if is_command(&text, "new", &bot_username) {
            let topic_id = nanoid::nanoid!(10);
            let _ = self
                .deps
                .storage
                .save_state(
                    format!("{}__{}", self.agent_id, channel.to_slug()),
                    serde_json::to_value(ChannelState {
                        active_topic: Some(topic_id.clone()),
                        show_thinking: false,
                        show_tool_calls: false,
                    })
                    .unwrap(),
                )
                .await;

            let _ = self
                .bot
                .clone()
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .send_message(chat_id, format!("switch to new session: **{}**", topic_id))
                .await?;
            return Ok(());
        }

        if is_command(&text, "session", &bot_username) {
            let parts: Vec<&str> = text.split_whitespace().collect();
            if parts.len() > 1 {
                let raw_topic_id = parts[1];
                let topic_id: Option<TopicId> = if raw_topic_id == "DEFAULT" {
                    None
                } else {
                    Some(raw_topic_id.to_string())
                };

                if let Ok(Some(_)) = self
                    .deps
                    .storage
                    .get_session_detail_by_topic(
                        self.agent_id.clone(),
                        channel.clone(),
                        topic_id.clone(),
                    )
                    .await
                {
                    let existing_state =
                        if let Ok(Some(value)) = self.deps.storage.get_state(key.clone()).await {
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
                        .deps
                        .storage
                        .save_state(
                            format!("{}__{}", self.agent_id, channel.to_slug()),
                            serde_json::to_value(ChannelState {
                                active_topic: topic_id,
                                show_thinking: existing_state.show_thinking,
                                show_tool_calls: existing_state.show_tool_calls,
                            })
                            .unwrap(),
                        )
                        .await;

                    let _ = self
                        .bot
                        .send_message(chat_id, format!("switch to session: **{}**", raw_topic_id))
                        .await?;
                } else {
                    let _ = self.bot.send_message(chat_id, "topic not found").await?;
                }
            } else {
                if let Ok(sessions) = self
                    .deps
                    .storage
                    .get_session_list(self.agent_id.clone(), Some(channel.clone()))
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
                    let _ = self.bot.send_message(chat_id, output).await?;
                }
            }
            return Ok(());
        }

        if is_command(&text, "abort", &bot_username) {
            let session = VizierSession(self.agent_id.clone(), channel.clone(), topic_id.clone());
            let _ = transport
                .send_request(
                    session,
                    VizierRequest {
                        trace: Default::default(),
                        client_tools: vec![],
                        client_categories: vec![],
                        client: None,
                        timestamp: Utc::now(),
                        user: user.clone(),
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
            return Ok(());
        }

        if is_command(&text, "checkpoint", &bot_username) {
            let session = VizierSession(self.agent_id.clone(), channel.clone(), topic_id.clone());
            let (response_tx, response_rx) = flume::unbounded();
            let _ = transport
                .send_request(
                    session,
                    VizierRequest {
                        trace: Default::default(),
                        client_tools: vec![],
                        client_categories: vec![],
                        client: None,
                        timestamp: Utc::now(),
                        user: user.clone(),
                        content: VizierRequestContent::Command("checkpoint".to_string()),
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
            let _ = self
                .bot
                .send_message(chat_id, "creating checkpoint...")
                .await?;
            let bot = self.bot.clone();
            tokio::spawn(async move {
                while let Ok(response) = response_rx.recv_async().await {
                    match response.content {
                        VizierResponseContent::Checkpoint { handover: Some(_) } => {
                            let _ = crate::utils::telegram::send_message(
                                &bot,
                                chat_id,
                                "✅ checkpoint saved".to_string(),
                            )
                            .await;
                            break;
                        }
                        VizierResponseContent::Checkpoint { handover: None } => {
                            let _ = crate::utils::telegram::send_message(
                                &bot,
                                chat_id,
                                "✅ lobotomy performed".to_string(),
                            )
                            .await;
                            break;
                        }
                        VizierResponseContent::Error { kind, message } => {
                            let kind_str = kind.label();
                            let _ = crate::utils::telegram::send_message(
                                &bot,
                                chat_id,
                                format!("**{}**: {}", kind_str, message),
                            )
                            .await;
                            break;
                        }
                        _ => {}
                    }
                }
            });
            return Ok(());
        }

        if is_command(&text, "lobotomy", &bot_username) {
            let session = VizierSession(self.agent_id.clone(), channel.clone(), topic_id.clone());
            let (response_tx, response_rx) = flume::unbounded();
            let _ = transport
                .send_request(
                    session,
                    VizierRequest {
                        trace: Default::default(),
                        client_tools: vec![],
                        client_categories: vec![],
                        client: None,
                        timestamp: Utc::now(),
                        user: user.clone(),
                        content: VizierRequestContent::Command("lobotomy".to_string()),
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
            let _ = self
                .bot
                .send_message(chat_id, "performing lobotomy...")
                .await?;
            let bot = self.bot.clone();
            tokio::spawn(async move {
                while let Ok(response) = response_rx.recv_async().await {
                    match response.content {
                        VizierResponseContent::Checkpoint { handover: Some(_) } => {
                            let _ = crate::utils::telegram::send_message(
                                &bot,
                                chat_id,
                                "✅ checkpoint saved".to_string(),
                            )
                            .await;
                            break;
                        }
                        VizierResponseContent::Checkpoint { handover: None } => {
                            let _ = crate::utils::telegram::send_message(
                                &bot,
                                chat_id,
                                "✅ lobotomy performed".to_string(),
                            )
                            .await;
                            break;
                        }
                        VizierResponseContent::Error { kind, message } => {
                            let kind_str = kind.label();
                            let _ = crate::utils::telegram::send_message(
                                &bot,
                                chat_id,
                                format!("**{}**: {}", kind_str, message),
                            )
                            .await;
                            break;
                        }
                        _ => {}
                    }
                }
            });
            return Ok(());
        }

        // `/<bot_username> …`, the bot's name as a whole first word.
        let by_name = text
            .strip_prefix(&format!("/{}", bot_username))
            .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace));
        let addressed = is_mention || by_name.is_some() || is_dm;
        // `/<skill> …` naming one of this agent's skills runs without a mention (spec 021).
        // `/skills` and unknown names still need one, so several bots in a group do not all
        // answer slash text meant for one of them.
        let should_respond = addressed || self.commands.names_skill(&text);

        let session = VizierSession(self.agent_id.clone(), channel.clone(), topic_id.clone());

        let (content, request_content) = if should_respond {
            let cleaned = if is_mention {
                text.replace(&format!("@{}", bot_username), "")
                    .trim()
                    .to_string()
            } else {
                by_name.unwrap_or(&text).trim().to_string()
            };
            (cleaned.clone(), VizierRequestContent::Chat(cleaned))
        } else {
            (text.clone(), VizierRequestContent::SilentRead(text))
        };

        let request = VizierRequest {
            trace: Default::default(),
            client_tools: vec![],
            client_categories: vec![],
            client: None,
            timestamp: chrono::Utc::now(),
            user,
            content: request_content,
            platform_message_id: Some(PlatformMessageId::Telegram(msg.id.0.into())),
            metadata,
            attachments,
            expect_audio_reply: None,
            scheduled_task: None,
            background_depth: 0,
            ..Default::default()
        };

        let renderer = TurnRenderer {
            bot: self.bot.clone(),
            file_manager: self.deps.file_manager.clone(),
            storage: self.deps.storage.clone(),
            agent_id: self.agent_id.clone(),
            chat_id,
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

        Ok(())
    }

    /// Post a turn a background report woke into the Telegram chat the work came from.
    async fn render_woken(
        bot: Bot,
        deps: VizierDependencies,
        session: VizierSession,
        frames: flume::Receiver<VizierResponse>,
    ) {
        let VizierChannelId::TelegramChannel(chat_id) = session.1 else {
            return;
        };
        let key = format!("{}__{}", session.0, session.1.to_slug());
        let state = match deps.storage.get_state(key).await {
            Ok(Some(value)) => serde_json::from_value::<ChannelState>(value).ok(),
            _ => None,
        };
        TurnRenderer {
            bot,
            file_manager: deps.file_manager.clone(),
            storage: deps.storage.clone(),
            agent_id: session.0.clone(),
            chat_id: ChatId(chat_id),
            show_thinking: state.as_ref().is_some_and(|s| s.show_thinking),
            show_tool_calls: state.as_ref().is_some_and(|s| s.show_tool_calls),
        }
        .render(frames)
        .await;
    }
}

/// Posts one turn's frames to a Telegram chat: a person's turn, or one a background report
/// woke (`channels::woken`).
struct TurnRenderer {
    bot: Bot,
    file_manager: crate::file_manager::FileManager,
    storage: std::sync::Arc<crate::storage::VizierStorage>,
    agent_id: String,
    chat_id: ChatId,
    show_thinking: bool,
    show_tool_calls: bool,
}

impl TurnRenderer {
    async fn render(self, response_rx: flume::Receiver<VizierResponse>) {
        let Self {
            bot,
            file_manager,
            storage,
            agent_id,
            chat_id: chat_id_copy,
            show_thinking,
            show_tool_calls,
        } = self;

        let mut typing_handle: Option<tokio::task::JoinHandle<()>> = None;

        while let Ok(response) = response_rx.recv_async().await {
            match response {
                VizierResponse {
                    content: VizierResponseContent::ThinkingStart,
                    ..
                } => {
                    if let Some(handle) = typing_handle.take() {
                        handle.abort();
                    }
                    let typing_bot = bot.clone();
                    let typing_chat_id = chat_id_copy;
                    let handle = tokio::spawn(async move {
                        loop {
                            let _ = typing_bot
                                .send_chat_action(typing_chat_id, ChatAction::Typing)
                                .await;
                            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                        }
                    });
                    typing_handle = Some(handle);
                }
                VizierResponse {
                    content: VizierResponseContent::ToolChoice { name, args },
                    ..
                } => {
                    if show_tool_calls {
                        let _ = crate::utils::telegram::send_message(
                            &bot,
                            chat_id_copy,
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
                        let _ = crate::utils::telegram::send_message(
                            &bot,
                            chat_id_copy,
                            format!("> {}", thought),
                        )
                        .await;
                    }
                }
                VizierResponse {
                    content: VizierResponseContent::Message { content, stats: _ },
                    attachments,
                    history_uid,
                    resumed,
                    ..
                } => {
                    if let Some(handle) = typing_handle.take() {
                        handle.abort();
                    }
                    let content =
                        crate::channels::continued_note(resumed, remove_think_tags(&content));
                    let mut posted =
                        crate::utils::telegram::send_message(&bot, chat_id_copy, content)
                            .await
                            .unwrap_or_default();

                    for attachment in &attachments {
                        match file_manager.resolve(attachment).await {
                            Ok((filename, bytes)) => {
                                let mime = get_mime_type(&filename);
                                let input_file =
                                    InputFile::memory(bytes).file_name(filename.clone());
                                let sent = if mime.starts_with("image/") {
                                    bot.send_photo(chat_id_copy, input_file)
                                        .await
                                        .map(|sent| sent.id)
                                        .map_err(|err| {
                                            format!("Failed to send photo attachment: {err:?}")
                                        })
                                } else {
                                    bot.send_document(chat_id_copy, input_file)
                                        .await
                                        .map(|sent| sent.id)
                                        .map_err(|err| {
                                            format!("Failed to send document attachment: {err:?}")
                                        })
                                };
                                match sent {
                                    Ok(id) => posted.push(id),
                                    Err(err) => {
                                        tracing::error!("{err}");
                                        let _ = bot
                                            .send_message(
                                                chat_id_copy,
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
                                let _ = bot
                                    .send_message(
                                        chat_id_copy,
                                        crate::channels::attachment_failure_notice(
                                            &attachment.filename,
                                        ),
                                    )
                                    .await;
                            }
                        }
                    }

                    link_reply(&storage, &agent_id, chat_id_copy, &posted, history_uid).await;
                }
                VizierResponse {
                    content: VizierResponseContent::AudioReply(audio_att, text, _),
                    history_uid,
                    ..
                } => {
                    if let Some(handle) = typing_handle.take() {
                        handle.abort();
                    }
                    let mut posted = vec![];
                    if let Some(content) = text {
                        let content = remove_think_tags(&content);
                        posted = crate::utils::telegram::send_message(&bot, chat_id_copy, content)
                            .await
                            .unwrap_or_default();
                    }
                    match file_manager.resolve(&audio_att).await {
                        Ok((filename, bytes)) => {
                            let input_file = InputFile::memory(bytes).file_name(filename.clone());
                            match bot.send_document(chat_id_copy, input_file).await {
                                Ok(sent) => posted.push(sent.id),
                                Err(err) => {
                                    tracing::error!("Failed to send audio reply: {:?}", err)
                                }
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

                    link_reply(&storage, &agent_id, chat_id_copy, &posted, history_uid).await;
                }
                VizierResponse {
                    content: VizierResponseContent::Abort,
                    ..
                } => {
                    if let Some(handle) = typing_handle.take() {
                        handle.abort();
                    }
                    let _ = crate::utils::telegram::send_message(
                        &bot,
                        chat_id_copy,
                        "thinking aborted".to_string(),
                    )
                    .await;
                }
                VizierResponse {
                    content: VizierResponseContent::Error { kind, message },
                    ..
                } => {
                    if let Some(handle) = typing_handle.take() {
                        handle.abort();
                    }
                    let kind_str = kind.label();
                    let _ = crate::utils::telegram::send_message(
                        &bot,
                        chat_id_copy,
                        format!("**{}**: {}", kind_str, message),
                    )
                    .await;
                }
                _ => {}
            }
        }

        if let Some(handle) = typing_handle.take() {
            handle.abort();
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ChannelState {
    active_topic: Option<TopicId>,
    #[serde(default)]
    show_thinking: bool,
    #[serde(default)]
    show_tool_calls: bool,
}

/// Link every message a reply was posted as to the history entry it renders, so reactions on
/// any chunk or attachment reach that entry. A failure is logged and never fails the send.
async fn link_reply(
    storage: &crate::storage::VizierStorage,
    agent_id: &str,
    chat_id: ChatId,
    posted: &[MessageId],
    history_uid: Option<String>,
) {
    let Some(uid) = history_uid else { return };
    if posted.is_empty() {
        return;
    }
    let ids: Vec<String> = posted.iter().map(|id| id.0.to_string()).collect();
    if let Err(err) = storage
        .link_platform_messages(
            agent_id,
            Platform::Telegram,
            &chat_id.0.to_string(),
            &ids,
            &uid,
        )
        .await
    {
        tracing::warn!("failed to link telegram reply {}: {:?}", uid, err);
    }
}

/// How a Telegram reaction is stored. Bots aren't given a custom emoji's name without another
/// call, so it is kept by id.
fn telegram_emoji_key(reaction: &ReactionType) -> String {
    match reaction {
        ReactionType::Emoji { emoji } => emoji.clone(),
        ReactionType::CustomEmoji { custom_emoji_id } => format!("custom_emoji:{custom_emoji_id}"),
        ReactionType::Paid => "⭐".to_string(),
    }
}

/// `(removed, added)` keys between one person's old and new reaction sets.
fn reaction_diff(old: &[ReactionType], new: &[ReactionType]) -> (Vec<String>, Vec<String>) {
    let old: Vec<String> = old.iter().map(telegram_emoji_key).collect();
    let new: Vec<String> = new.iter().map(telegram_emoji_key).collect();
    let removed = old.iter().filter(|k| !new.contains(k)).cloned().collect();
    let added = new.iter().filter(|k| !old.contains(k)).cloned().collect();
    (removed, added)
}

/// Menu commands: names of 1–32 `a-z0-9_`, descriptions up to 256 characters, at most 100.
/// `-` cannot appear, so a skill shows with `_` (`/release_notes`), which resolves back to it.
fn telegram_rules() -> PlatformRules {
    PlatformRules {
        name_ok: |name| {
            commands::name_ok(name, |c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'
            })
        },
        convert: |name| name.to_lowercase().replace('-', "_"),
        max_commands: 100,
        max_description: 256,
        builtins: vec![
            CommandEntry::new("ping", "a simple ping"),
            CommandEntry::new("new", "create fresh new session"),
            CommandEntry::new("session", "list or select session"),
            CommandEntry::new("abort", "abort current thinking"),
            CommandEntry::new("checkpoint", "save checkpoint with handover summary"),
            CommandEntry::new("lobotomy", "save checkpoint without handover (clean break)"),
            CommandEntry::new("thinking", "toggle showing thinking output"),
            CommandEntry::new("tool_calls", "toggle showing tool call details"),
            CommandEntry::new("clear_files", "clear this conversation's session files"),
        ],
    }
}

/// The message is the built-in command `name`: its whole first word, with an `@bot` suffix
/// only when it names this bot. A prefix is not enough, or `/newsletter` would be `/new`.
fn is_command(text: &str, name: &str, bot_username: &str) -> bool {
    let first = text.split_whitespace().next().unwrap_or("");
    let Some(command) = first.strip_prefix('/') else {
        return false;
    };
    match command.split_once('@') {
        Some((command, bot)) => command == name && bot.eq_ignore_ascii_case(bot_username),
        None => command == name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_commands_match_the_whole_word() {
        assert!(is_command("/new", "new", "mybot"));
        assert!(is_command("/session abc", "session", "mybot"));
        assert!(is_command("/abort@MyBot", "abort", "mybot"));
        assert!(!is_command("/abort@otherbot", "abort", "mybot"));
        assert!(!is_command("/newsletter", "new", "mybot"));
        assert!(!is_command("hello /new", "new", "mybot"));
    }

    fn emoji(e: &str) -> ReactionType {
        ReactionType::Emoji {
            emoji: e.to_string(),
        }
    }

    #[test]
    fn a_diff_gives_removals_and_additions() {
        assert_eq!(
            reaction_diff(&[], &[emoji("👍")]),
            (vec![], vec!["👍".to_string()])
        );
        assert_eq!(
            reaction_diff(&[emoji("👍")], &[]),
            (vec!["👍".to_string()], vec![])
        );
        assert_eq!(
            reaction_diff(&[emoji("👍")], &[emoji("🔥")]),
            (vec!["👍".to_string()], vec!["🔥".to_string()])
        );
        assert_eq!(
            reaction_diff(&[emoji("👍"), emoji("🔥")], &[emoji("🔥"), emoji("👍")]),
            (vec![], vec![])
        );
    }

    #[test]
    fn custom_and_paid_reactions_have_stable_keys() {
        assert_eq!(
            telegram_emoji_key(&ReactionType::CustomEmoji {
                custom_emoji_id: "123".into()
            }),
            "custom_emoji:123"
        );
        assert_eq!(telegram_emoji_key(&ReactionType::Paid), "⭐");
    }
}
