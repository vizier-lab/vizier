use anyhow::Result;

use crate::{
    channels::http::HTTPChannel, config::HTTPChannelConfig, dependencies::VizierDependencies,
};

pub mod discord;
pub mod http;
pub mod reactions;
pub mod readers;
pub mod telegram;
pub mod woken;

/// A reader error that retrying won't fix, such as a revoked token: the reader is marked
/// failed rather than restarted, until a config change (an agent update) starts it afresh.
#[derive(Debug)]
pub struct FatalChannelError(pub String);

impl std::fmt::Display for FatalChannelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FatalChannelError {}

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

    /// Runs the HTTP channel, returning only if it stops, so that `run_server` sees it stop
    /// (#51). Without one configured this never returns.
    pub async fn run(&mut self) -> Result<()> {
        let Some(http_config) = &self.http_config else {
            futures::future::pending::<()>().await;
            return Ok(());
        };
        let http = HTTPChannel::new(http_config.clone(), self.deps.clone())?;
        http.run().await?;
        Err(anyhow::anyhow!("the http server stopped"))
    }
}
