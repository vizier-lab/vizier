use std::collections::HashMap;

use anyhow::Result;
use tokio::task::JoinHandle;

use crate::{
    channels::{
        discord::{DiscordChannelReader, DiscordChannelWriter},
        http::HTTPChannel,
        telegram::{TelegramChannelReader, TelegramChannelWriter},
    },
    config::HTTPChannelConfig,
    dependencies::VizierDependencies,
    schema::{AgentConfig, ChannelCommand},
    storage::agent::AgentStorage,
};

pub mod discord;
pub mod http;
pub mod telegram;

pub trait VizierChannel {
    async fn run(&mut self) -> Result<()>;
}

struct AgentChannels {
    discord_reader: Option<JoinHandle<()>>,
    discord_writer: Option<JoinHandle<()>>,
    telegram_reader: Option<JoinHandle<()>>,
    telegram_writer: Option<JoinHandle<()>>,
}

pub struct VizierChannels {
    http_config: Option<HTTPChannelConfig>,
    deps: VizierDependencies,
    agent_channels: HashMap<String, AgentChannels>,
}

impl VizierChannels {
    pub fn new(http_config: Option<HTTPChannelConfig>, deps: VizierDependencies) -> Result<Self> {
        Ok(Self {
            http_config,
            deps,
            agent_channels: HashMap::new(),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let agents = self.deps.storage.list_agents().await.unwrap_or_default();

        for (agent_id, config) in &agents {
            self.spawn_agent_channels(agent_id, config);
        }

        if let Some(http_config) = &self.http_config {
            let mut http = HTTPChannel::new(http_config.clone(), self.deps.clone())?;
            tokio::spawn(async move {
                if let Err(e) = http.run().await {
                    tracing::error!("Err{:?}", e);
                }
            });
        }

        loop {
            match self.deps.transport.recv_channel_command().await {
                Ok(cmd) => match cmd {
                    ChannelCommand::AgentCreated { agent_id, config } => {
                        tracing::info!(
                            "channel command: agent '{}' created, spawning channels",
                            agent_id
                        );
                        self.spawn_agent_channels(&agent_id, &config);
                    }
                    ChannelCommand::AgentUpdated { agent_id, config } => {
                        tracing::info!(
                            "channel command: agent '{}' updated, reconciling channels",
                            agent_id
                        );
                        self.abort_agent_channels(&agent_id);
                        self.spawn_agent_channels(&agent_id, &config);
                    }
                    ChannelCommand::AgentDeleted { agent_id } => {
                        tracing::info!(
                            "channel command: agent '{}' deleted, stopping channels",
                            agent_id
                        );
                        self.abort_agent_channels(&agent_id);
                    }
                },
                Err(e) => {
                    tracing::error!("failed to receive channel command: {:?}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }
    }

    fn spawn_agent_channels(&mut self, agent_id: &str, config: &AgentConfig) {
        if self.agent_channels.contains_key(agent_id) {
            return;
        }

        let mut channels = AgentChannels {
            discord_reader: None,
            discord_writer: None,
            telegram_reader: None,
            telegram_writer: None,
        };

        if let Some(token) = &config.discord_token
            && !token.is_empty()
        {
            let agent_id_owned = agent_id.to_string();
            let token_owned = token.clone();
            let deps = self.deps.clone();
            channels.discord_reader = Some(tokio::spawn(async move {
                match DiscordChannelReader::new(agent_id_owned.clone(), token_owned, deps).await {
                    Ok(mut reader) => {
                        if let Err(e) = reader.run().await {
                            tracing::error!("discord reader error: {:?}", e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("failed to create discord reader: {:?}", e);
                    }
                }
            }));

            let agent_id_owned = agent_id.to_string();
            let token_owned = token.clone();
            let transport = self.deps.transport.clone();
            channels.discord_writer = Some(tokio::spawn(async move {
                let mut writer = DiscordChannelWriter::new(agent_id_owned, token_owned, transport);
                if let Err(e) = writer.run().await {
                    tracing::error!("discord writer error: {:?}", e);
                }
            }));
        }

        if let Some(token) = &config.telegram_token
            && !token.is_empty()
        {
            let agent_id_owned = agent_id.to_string();
            let token_owned = token.clone();
            let deps = self.deps.clone();
            channels.telegram_reader = Some(tokio::spawn(async move {
                match TelegramChannelReader::new(agent_id_owned.clone(), token_owned, deps).await {
                    Ok(mut reader) => {
                        if let Err(e) = reader.run().await {
                            tracing::error!("telegram reader error: {:?}", e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("failed to create telegram reader: {:?}", e);
                    }
                }
            }));

            let agent_id_owned = agent_id.to_string();
            let token_owned = token.clone();
            let transport = self.deps.transport.clone();
            channels.telegram_writer = Some(tokio::spawn(async move {
                let mut writer =
                    TelegramChannelWriter::new(agent_id_owned, token_owned, transport);
                if let Err(e) = writer.run().await {
                    tracing::error!("telegram writer error: {:?}", e);
                }
            }));
        }

        let has_any = channels.discord_reader.is_some()
            || channels.discord_writer.is_some()
            || channels.telegram_reader.is_some()
            || channels.telegram_writer.is_some();

        if has_any {
            self.agent_channels.insert(agent_id.to_string(), channels);
        }
    }

    fn abort_agent_channels(&mut self, agent_id: &str) {
        if let Some(channels) = self.agent_channels.remove(agent_id) {
            if let Some(h) = channels.discord_reader {
                h.abort();
            }
            if let Some(h) = channels.discord_writer {
                h.abort();
            }
            if let Some(h) = channels.telegram_reader {
                h.abort();
            }
            if let Some(h) = channels.telegram_writer {
                h.abort();
            }
        }
    }
}
