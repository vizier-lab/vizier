use anyhow::Result;

use crate::{
    channels::{
        discord::{DiscordChannelReader, DiscordChannelWriter},
        http::HTTPChannel,
        telegram::{TelegramChannelReader, TelegramChannelWriter},
    },
    config::HTTPChannelConfig,
    dependencies::VizierDependencies,
    storage::agent::AgentStorage,
};

pub mod discord;
pub mod http;
pub mod telegram;

pub trait VizierChannel {
    async fn run(&mut self) -> Result<()>;
}

pub struct VizierChannels {
    http_config: Option<HTTPChannelConfig>,
    deps: VizierDependencies,
}

impl VizierChannels {
    pub fn new(http_config: Option<HTTPChannelConfig>, deps: VizierDependencies) -> Result<Self> {
        Ok(Self { http_config, deps })
    }

    pub async fn run(&self) -> Result<()> {
        let agents = self.deps.storage.list_agents().await.unwrap_or_default();

        // Spawn Discord channels
        let discord_agents: Vec<(String, String)> = agents
            .iter()
            .filter_map(|(id, config)| {
                config.discord_token.as_ref().map(|t| (id.clone(), t.clone()))
            })
            .collect();

        if !discord_agents.is_empty() {
            for (agent_id, token) in &discord_agents {
                let agent_id = agent_id.clone();
                let token = token.clone();
                let deps = self.deps.clone();
                tokio::spawn(async move {
                    let mut discord_reader =
                        DiscordChannelReader::new(agent_id.clone(), token, deps.clone())
                            .await
                            .unwrap();

                    if let Err(e) = discord_reader.run().await {
                        tracing::error!("Err{:?}", e)
                    }
                });
            }

            let transport = self.deps.transport.clone();
            let token_map: std::collections::HashMap<String, String> =
                discord_agents.into_iter().collect();
            tokio::spawn(async move {
                let mut discord_writer = DiscordChannelWriter::new(transport.clone(), token_map);

                if let Err(e) = discord_writer.run().await {
                    tracing::error!("Err{:?}", e)
                }
            });
        }

        // Spawn HTTP channel
        if let Some(http_config) = &self.http_config {
            let mut http = HTTPChannel::new(http_config.clone(), self.deps.clone())?;

            tokio::spawn(async move {
                if let Err(e) = http.run().await {
                    tracing::error!("Err{:?}", e);
                }
            });
        }

        // Spawn Telegram channels
        let telegram_agents: Vec<(String, String)> = agents
            .iter()
            .filter_map(|(id, config)| {
                config.telegram_token.as_ref().map(|t| (id.clone(), t.clone()))
            })
            .collect();

        if !telegram_agents.is_empty() {
            for (agent_id, token) in &telegram_agents {
                let agent_id = agent_id.clone();
                let token = token.clone();
                let deps = self.deps.clone();
                tokio::spawn(async move {
                    let mut telegram_reader =
                        TelegramChannelReader::new(agent_id.clone(), token, deps.clone())
                            .await
                            .unwrap();

                    if let Err(e) = telegram_reader.run().await {
                        tracing::error!("Err{:?}", e)
                    }
                });
            }

            let transport = self.deps.transport.clone();
            let token_map: std::collections::HashMap<String, String> =
                telegram_agents.into_iter().collect();
            tokio::spawn(async move {
                let mut telegram_writer = TelegramChannelWriter::new(transport.clone(), token_map);

                if let Err(e) = telegram_writer.run().await {
                    tracing::error!("Err{:?}", e)
                }
            });
        }

        Ok(())
    }
}
