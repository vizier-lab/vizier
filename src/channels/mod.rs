use anyhow::Result;

use crate::{
    channels::http::HTTPChannel, config::HTTPChannelConfig, dependencies::VizierDependencies,
};

pub mod discord;
pub mod http;
pub mod reaction_store;
pub mod telegram;

#[async_trait::async_trait]
pub trait VizierChannel {
    async fn run(&self) -> Result<()>;
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

pub struct VizierChannels {
    http_config: Option<HTTPChannelConfig>,
    deps: VizierDependencies,
}

impl VizierChannels {
    pub fn new(http_config: Option<HTTPChannelConfig>, deps: VizierDependencies) -> Result<Self> {
        Ok(Self { http_config, deps })
    }

    pub async fn run(&mut self) -> Result<()> {
        if let Some(http_config) = &self.http_config {
            let mut http = HTTPChannel::new(http_config.clone(), self.deps.clone())?;
            tokio::spawn(async move {
                if let Err(e) = http.run().await {
                    tracing::error!("Err{:?}", e);
                }
            });
        }

        futures::future::pending::<()>().await;
        Ok(())
    }
}
