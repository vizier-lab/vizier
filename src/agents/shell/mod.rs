use std::sync::Arc;

use anyhow::Result;

use crate::{
    config::shell::ShellConfig,
    agents::shell::{docker::DockerShell, local::LocalShell},
};

pub mod docker;
pub mod local;

#[async_trait::async_trait]
pub trait ShellProvider {
    async fn exec(&self, commands: String) -> Result<String>;
}

pub struct VizierShell(Arc<Box<dyn ShellProvider + Sync + Send + 'static>>);

impl VizierShell {
    pub fn build<Shell: ShellProvider + Sync + Send + 'static>(shell: Shell) -> Self {
        Self(Arc::new(Box::new(shell)))
    }

    pub async fn new(config: &ShellConfig) -> Result<Self> {
        Ok(match config {
            ShellConfig::Docker(docker) => Self::build(DockerShell::new(docker.clone()).await?),
            ShellConfig::Local(local) => Self::build(LocalShell::new(local.clone()).await?),
        })
    }
}

#[async_trait::async_trait]
impl ShellProvider for VizierShell {
    async fn exec(&self, commands: String) -> Result<String> {
        self.0.exec(commands).await
    }
}
