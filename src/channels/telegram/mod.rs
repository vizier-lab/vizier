use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use teloxide::Bot;
use teloxide::prelude::*;
use teloxide::types::ChatAction;

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

pub struct TelegramChannelReader {
    bot: Bot,
    token: String,
    agent_id: String,
    deps: VizierDependencies,
    offset: i64,
}

impl TelegramChannelReader {
    pub async fn new(
        agent_id: String,
        token: String,
        deps: VizierDependencies,
    ) -> Result<Self> {
        let bot = Bot::new(token.clone());

        Ok(Self {
            bot,
            token,
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
                            tracing::error!("Error handling update: {:?}", e);
                        }
                    }
                }
                Err(_e) => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
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
            _ => return Ok(()),
        }

        Ok(())
    }

    async fn handle_message(&self, msg: Message) -> Result<()> {

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
                if let Ok(file) = self.bot.get_file(&file_id).await {
                    let url = format!(
                        "https://api.telegram.org/file/bot{}/{}",
                        self.token, file.path
                    );
                    let bytes = reqwest::get(&url).await?.bytes().await?.to_vec();
                    let file_record = self.deps.transport.send_file_upload(format!("photo_{}.jpg", file_id), bytes).await
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
            if let Ok(file) = self.bot.get_file(&file_id).await {
                let url = format!(
                    "https://api.telegram.org/file/bot{}/{}",
                    self.token, file.path
                );
                let filename = doc
                    .file_name
                    .clone()
                    .unwrap_or_else(|| format!("document_{}", file_id));
                let bytes = reqwest::get(&url).await?.bytes().await?.to_vec();
                let file_record = self.deps.transport.send_file_upload(filename.clone(), bytes).await
                    .map_err(|e| VizierError(e.to_string()))?;
                attachments.push(VizierAttachment { filename, content: VizierAttachmentContent::Local(file_record.url) });
            }
        }

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

        if text.starts_with("/abort") {
            let session = VizierSession(
                self.agent_id.clone(),
                channel.clone(),
                topic_id.clone(),
            );
            let _ = transport
                .send_request(
                    session,
                    VizierRequest {
                        timestamp: Utc::now(),
                        user: user.clone(),
                        content: VizierRequestContent::Command("abort".to_string()),
                        platform_message_id: None,
                        metadata: serde_json::json!({}),
                        attachments: vec![],
                    },
                    None,
                )
                .await;
            return Ok(());
        }

        let should_respond = is_mention || text.starts_with(&format!("/{}", bot_username)) || is_dm;

        let session = VizierSession(
            self.agent_id.clone(),
            channel.clone(),
            topic_id.clone(),
        );

        let (content, request_content) = if should_respond {
            let cleaned = if is_mention {
                text.replace(&format!("@{}", bot_username), "")
                    .trim()
                    .to_string()
            } else {
                text.replace(&format!("/{}", bot_username), "")
                    .trim()
                    .to_string()
            };
            (cleaned.clone(), VizierRequestContent::Chat(cleaned))
        } else {
            (text.clone(), VizierRequestContent::SilentRead(text))
        };

        let request = VizierRequest {
            timestamp: chrono::Utc::now(),
            user,
            content: request_content,
            platform_message_id: Some(PlatformMessageId::Telegram(msg.id.0.into())),
            metadata,
            attachments,
        };

        let bot = self.bot.clone();
        let chat_id_copy = chat_id;

        tokio::spawn(async move {
            let (response_tx, response_rx) = flume::unbounded();

            if let Err(err) = transport
                .send_request(session.clone(), request, Some(response_tx))
                .await
            {
                tracing::error!("{}", err);
                return;
            }

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
                        let _ = crate::utils::telegram::send_message(
                            &bot,
                            chat_id_copy,
                            crate::utils::format_thinking(&name, &args),
                        )
                        .await;
                    }
                    VizierResponse {
                        content: VizierResponseContent::Thinking(thought),
                        ..
                    } => {
                        let _ = crate::utils::telegram::send_message(
                            &bot,
                            chat_id_copy,
                            format!("> {}", thought),
                        )
                        .await;
                    }
                    VizierResponse {
                        content:
                            VizierResponseContent::Message {
                                content, stats: _
                            },
                        ..
                    } => {
                        if let Some(handle) = typing_handle.take() {
                            handle.abort();
                        }
                        let content = remove_think_tags(&content);
                        let _ =
                            crate::utils::telegram::send_message(&bot, chat_id_copy, content).await;
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
                    _ => {}
                }
            }

            if let Some(handle) = typing_handle.take() {
                handle.abort();
            }
        });

        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ChannelState {
    active_topic: Option<TopicId>,
}
