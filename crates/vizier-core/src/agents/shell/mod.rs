use std::sync::Arc;

use anyhow::Result;

#[cfg(feature = "docker-shell")]
use crate::agents::shell::docker::DockerShell;
use crate::{
    agents::shell::local::LocalShell,
    config::shell::{LocalShellConfig, ShellConfig},
};

#[cfg(feature = "docker-shell")]
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
            #[cfg(feature = "docker-shell")]
            ShellConfig::Docker(docker) => Self::build(DockerShell::new(docker.clone()).await?),
            #[cfg(not(feature = "docker-shell"))]
            ShellConfig::Docker(_) => {
                anyhow::bail!("docker shell support is not compiled into this build")
            }
            ShellConfig::Local(local) => Self::build(LocalShell::new(local.clone()).await?),
        })
    }

    /// A local shell from an addon's settings, built synchronously (spec 040).
    pub fn local(config: LocalShellConfig) -> Self {
        Self::build(LocalShell::from_config(config))
    }
}

#[async_trait::async_trait]
impl ShellProvider for VizierShell {
    async fn exec(&self, commands: String) -> Result<String> {
        self.0.exec(commands).await
    }
}
