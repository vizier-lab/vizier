use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use teloxide::Bot;
use teloxide::prelude::*;
use teloxide::types::ChatAction;

use crate::channels::VizierChannel;
use crate::config::TelegramChannelConfig;
use crate::dependencies::VizierDependencies;
use crate::schema::{
    TopicId, VizierChannelId, VizierRequest, VizierRequestContent, VizierResponse, VizierSession,
};
use crate::storage::session::SessionStorage;
use crate::storage::state::StateStorage;
use crate::transport::VizierTransport;
use crate::utils::remove_think_tags;

pub struct TelegramChannelReader {
    bot: Bot,
    agent_id: String,
    deps: VizierDependencies,
    offset: i64,
}

impl TelegramChannelReader {
    pub async fn new(
        agent_id: String,
        config: TelegramChannelConfig,
        deps: VizierDependencies,
    ) -> Result<Self> {
        let bot = Bot::new(config.token);

        Ok(Self {
            bot,
            agent_id,
            deps,
            offset: 0,
        })
    }
}

impl VizierChannel for TelegramChannelReader {
    async fn run(&mut self) -> Result<()> {
        loop {
            let updates = self
                .bot
                .get_updates()
                .offset(self.offset as i32)
                .timeout(30)
                .await;

            match updates {
                Ok(updates) => {
                    for update in updates {
                        self.offset = update.id.0 as i64 + 1;
                        if let Err(e) = self.handle_update(update).await {
                            log::error!("Error handling update: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }
    }
}

impl TelegramChannelReader {
    async fn handle_update(&self, update: Update) -> Result<()> {
        let kind = &update.kind;

        let msg = match kind {
            teloxide::types::UpdateKind::Message(msg) => msg.clone(),
            teloxide::types::UpdateKind::EditedMessage(msg) => msg.clone(),
            _ => return Ok(()),
        };

        let chat_id = msg.chat.id;
        let channel = VizierChannelId::TelegramChannel(chat_id.0);

        let is_dm = msg.chat.is_private();

        let key = format!("{}__{}", self.agent_id, channel.to_slug());
        let topic_id = if let Ok(Some(value)) = self.deps.storage.get_state(key).await {
            let state = serde_json::from_value::<ChannelState>(value).unwrap();
            state.active_topic
        } else {
            None
        };

        let text = msg.text().unwrap_or("").to_string();
        let bot_username = self.bot.get_me().await?.username().to_string();

        let is_mention = text.starts_with(&format!("@{}", bot_username))
            || text.contains(&format!("@{}", bot_username));

        let replied_to = msg.reply_to_message().as_ref().map(|m| m.id.to_string());
        let message_id = msg.id.to_string();
        let user_full_name = msg
            .from
            .map(|u| u.username.clone().unwrap())
            .unwrap_or_else(|| "Unknown".into());

        let metadata = serde_json::json!({
            "sent_at": Utc::now().to_string(),
            "is_reply_message": replied_to.is_some(),
            "replied_message_id": replied_to,
            "message_id": message_id,
            "telegram_chat_id": chat_id.to_string(),
            "is_dm": is_dm,
        });

        let user = format!("@{} (TelegramUser: {})", user_full_name, chat_id.0);

        let transport = self.deps.transport.clone();

        if text.starts_with("/ping") {
            let _ = self.bot.send_message(chat_id, "Pong!").await?;
            return Ok(());
        }

        if text.starts_with("/new") {
            let topic_id = nanoid::nanoid!(10);
            let _ = self
                .deps
                .storage
                .save_state(
                    format!("{}__{}", self.agent_id, channel.to_slug()),
                    serde_json::to_value(ChannelState {
                        active_topic: Some(topic_id.clone()),
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

        if text.starts_with("/session") {
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
                    let _ = self
                        .deps
                        .storage
                        .save_state(
                            format!("{}__{}", self.agent_id, channel.to_slug()),
                            serde_json::to_value(ChannelState {
                                active_topic: topic_id,
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
                    .get_session_list(self.agent_id.clone(), channel.clone())
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

        let should_respond = is_mention || text.starts_with(&format!("/{}", bot_username)) || is_dm;

        if should_respond {
            let content = if is_mention {
                text.replace(&format!("@{}", bot_username), "")
                    .trim()
                    .to_string()
            } else {
                text.replace(&format!("/{}", bot_username), "")
                    .trim()
                    .to_string()
            };

            let transport = transport.clone();
            let agent_id = self.agent_id.clone();
            let channel = channel.clone();
            tokio::spawn(async move {
                if let Err(err) = transport
                    .send_request(
                        VizierSession(agent_id, channel, topic_id),
                        VizierRequest {
                            user,
                            content: VizierRequestContent::Chat(content),
                            metadata,
                        },
                    )
                    .await
                {
                    log::error!("{}", err);
                }
            });
        } else {
            let transport = transport.clone();
            let agent_id = self.agent_id.clone();
            let channel = channel.clone();
            tokio::spawn(async move {
                if let Err(err) = transport
                    .send_request(
                        VizierSession(agent_id, channel, topic_id),
                        VizierRequest {
                            user,
                            content: VizierRequestContent::SilentRead(text),
                            metadata,
                        },
                    )
                    .await
                {
                    log::error!("{}", err);
                }
            });
        }

        Ok(())
    }
}

pub struct TelegramChannelWriter {
    transport: VizierTransport,
    bots: HashMap<String, Bot>,
}

impl TelegramChannelWriter {
    pub fn new(transport: VizierTransport, config: HashMap<String, TelegramChannelConfig>) -> Self {
        let bots = config
            .into_iter()
            .map(|(agent_id, cfg)| (agent_id, Bot::new(cfg.token)))
            .collect();

        Self { transport, bots }
    }
}

impl VizierChannel for TelegramChannelWriter {
    async fn run(&mut self) -> Result<()> {
        let mut recv = self.transport.subscribe_response().await?;
        let bots = self.bots.clone();

        let mut typing_handles: HashMap<i64, tokio::task::JoinHandle<()>> = HashMap::new();

        let _ = tokio::spawn(async move {
            loop {
                if let Ok((
                    VizierSession(agent_id, VizierChannelId::TelegramChannel(chat_id), _),
                    res,
                )) = recv.recv().await
                {
                    let bot = bots.get(&agent_id).unwrap().clone();
                    let chat_id = ChatId(chat_id);

                    match res {
                        VizierResponse::ThinkingStart => {
                            if let Some(handle) = typing_handles.remove(&chat_id.0) {
                                handle.abort();
                            }
                            let typing_bot = bot.clone();
                            let typing_chat_id = chat_id;
                            let typing_task = tokio::spawn(async move {
                                loop {
                                    let _ = typing_bot.send_chat_action(typing_chat_id, ChatAction::Typing).await;
                                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                                }
                            });
                            typing_handles.insert(chat_id.0, typing_task);
                        }
                        VizierResponse::ToolChoice { name, args } => {
                            let _ = crate::utils::telegram::send_message(
                                &bot,
                                chat_id,
                                crate::utils::format_thinking(&name, &args),
                            )
                            .await;
                        }
                        VizierResponse::Thinking(thought) => {
                            let _ = crate::utils::telegram::send_message(
                                &bot,
                                chat_id,
                                format!("> {}", thought),
                            )
                            .await;
                        }
                        VizierResponse::Message { content, stats: _ } => {
                            if let Some(handle) = typing_handles.remove(&chat_id.0) {
                                handle.abort();
                            }
                            let content = remove_think_tags(&content.clone());
                            let _ =
                                crate::utils::telegram::send_message(&bot, chat_id, content).await;
                        }
                        VizierResponse::Abort => {
                            if let Some(handle) = typing_handles.remove(&chat_id.0) {
                                handle.abort();
                            }
                            let _ = crate::utils::telegram::send_message(
                                &bot,
                                chat_id,
                                "thinking aborted".to_string(),
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

#[derive(Debug, Deserialize, Serialize)]
struct ChannelState {
    active_topic: Option<TopicId>,
}

