use anyhow::Result;

use crate::{
    channels::{
        discord::{DiscordChannelReader, DiscordChannelWriter},
        http::HTTPChannel,
        telegram::{TelegramChannelReader, TelegramChannelWriter},
    },
    config::ChannelsConfig,
    dependencies::VizierDependencies,
};

pub mod discord;
pub mod http;
pub mod telegram;

pub trait VizierChannel {
    async fn run(&mut self) -> Result<()>;
}

pub struct VizierChannels {
    config: ChannelsConfig,
    deps: VizierDependencies,
}

impl VizierChannels {
    pub fn new(config: ChannelsConfig, deps: VizierDependencies) -> Result<Self> {
        Ok(Self { config, deps })
    }

    pub async fn run(&self) -> Result<()> {
        if let Some(discord_configs) = &self.config.discord {
            for (agent_id, discord_config) in discord_configs.iter() {
                let agent_id = agent_id.clone();
                let deps = self.deps.clone();
                let reader_discord_config = discord_config.clone();
                tokio::spawn(async move {
                    let mut discord_reader = DiscordChannelReader::new(
                        agent_id.clone(),
                        reader_discord_config.clone(),
                        deps.clone(),
                    )
                    .await
                    .unwrap();

                    if let Err(e) = discord_reader.run().await {
                        tracing::error!("Err{:?}", e)
                    }
                });
            }

            let transport = self.deps.transport.clone();
            let discord_configs = discord_configs.clone();
            tokio::spawn(async move {
                let mut discord_writer =
                    DiscordChannelWriter::new(transport.clone(), discord_configs.clone());

                if let Err(e) = discord_writer.run().await {
                    tracing::error!("Err{:?}", e)
                }
            });
        }

        if let Some(http) = &self.config.http {
            let mut http = HTTPChannel::new(http.clone(), self.deps.clone())?;

            tokio::spawn(async move {
                if let Err(e) = http.run().await {
                    tracing::error!("Err{:?}", e);
                }
            });
        }

        if let Some(telegram_configs) = &self.config.telegram {
            for (agent_id, telegram_config) in telegram_configs.iter() {
                let agent_id = agent_id.clone();
                let deps = self.deps.clone();
                let reader_telegram_config = telegram_config.clone();
                tokio::spawn(async move {
                    let mut telegram_reader = TelegramChannelReader::new(
                        agent_id.clone(),
                        reader_telegram_config.clone(),
                        deps.clone(),
                    )
                    .await
                    .unwrap();

                    if let Err(e) = telegram_reader.run().await {
                        tracing::error!("Err{:?}", e)
                    }
                });
            }

            let transport = self.deps.transport.clone();
            let telegram_configs = telegram_configs.clone();
            tokio::spawn(async move {
                let mut telegram_writer =
                    TelegramChannelWriter::new(transport.clone(), telegram_configs.clone());

                if let Err(e) = telegram_writer.run().await {
                    tracing::error!("Err{:?}", e)
                }
            });
        }

        Ok(())
    }
}
