use std::sync::Arc;

use anyhow::Result;

use crate::dependencies::VizierDependencies;
use crate::mcp::VizierMcpClients;
use crate::schema::{GlobalCommand, GlobalCommandResult};
use crate::shell::VizierShell;

pub struct VizierGlobalResources {
    deps: VizierDependencies,
}

impl VizierGlobalResources {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            let cmd = self.deps.transport.recv_global_command().await?;
            match cmd {
                GlobalCommand::ReloadMcp { config, resp } => {
                    let result = self.handle_reload_mcp(config).await;
                    let _ = resp.send(result);
                }
                GlobalCommand::ReloadShell { config, resp } => {
                    let result = self.handle_reload_shell(config).await;
                    let _ = resp.send(result);
                }
            }
        }
    }

    async fn handle_reload_mcp(
        &self,
        config: std::collections::HashMap<String, crate::config::tools::mcp::McpClientConfig>,
    ) -> GlobalCommandResult {
        let vizier_config = (*self.deps.config).clone();

        let mut new_config = vizier_config;
        new_config.tools.mcp_servers = config;

        match VizierMcpClients::new(new_config).await {
            Ok(new_clients) => {
                self.deps.mcp_clients.store(Arc::new(new_clients));
                tracing::info!("MCP clients reloaded successfully");
                GlobalCommandResult::Ok("MCP clients reloaded".into())
            }
            Err(e) => {
                tracing::error!("failed to reload MCP clients: {}", e);
                GlobalCommandResult::Error(format!("failed to reload MCP clients: {}", e))
            }
        }
    }

    async fn handle_reload_shell(
        &self,
        config: crate::config::shell::ShellConfig,
    ) -> GlobalCommandResult {
        match VizierShell::new(&config).await {
            Ok(new_shell) => {
                self.deps.shell.store(Arc::new(new_shell));
                tracing::info!("Shell config reloaded successfully");
                GlobalCommandResult::Ok("Shell config reloaded".into())
            }
            Err(e) => {
                tracing::error!("failed to reload shell config: {}", e);
                GlobalCommandResult::Error(format!("failed to reload shell config: {}", e))
            }
        }
    }
}
