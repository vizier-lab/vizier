use anyhow::Result;

use crate::{
    channels::{
        discord::{DiscordChannelReader, DiscordChannelWriter},
        http::HTTPChannel,
    },
    config::ChannelsConfig,
    dependencies::VizierDependencies,
};

pub mod discord;
pub mod http;

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
                        log::error!("Err{:?}", e)
                    }
                });
            }

            let transport = self.deps.transport.clone();
            let discord_configs = discord_configs.clone();
            tokio::spawn(async move {
                let mut discord_writer =
                    DiscordChannelWriter::new(transport.clone(), discord_configs.clone());

                if let Err(e) = discord_writer.run().await {
                    log::error!("Err{:?}", e)
                }
            });
        }

        if let Some(http) = &self.config.http {
            let mut http = HTTPChannel::new(http.clone(), self.deps.clone())?;

            tokio::spawn(async move {
                if let Err(e) = http.run().await {
                    log::error!("Err{:?}", e);
                }
            });
        }

        Ok(())
    }
}
