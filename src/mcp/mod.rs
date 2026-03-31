use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use rig::completion::ToolDefinition;
use tokio::process::Command;

use rmcp::{
    RoleClient, Service, ServiceExt,
    model::{CallToolRequestParams, ClientCapabilities, ClientInfo, Implementation},
    service::RunningService,
    transport::{ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess},
};

use crate::config::{VizierConfig, tools::mcp::McpClientConfig};

pub struct VizierMcpClients {
    pub clients: HashMap<String, Arc<VizierMcp>>,
}

impl VizierMcpClients {
    pub async fn new(config: VizierConfig) -> Result<Self> {
        let mut clients = HashMap::new();
        for (server_name, mcp_config) in config.tools.mcp_servers.iter() {
            clients.insert(server_name.clone(), Arc::new(mcp_config.to_client().await?));
        }

        Ok(Self { clients })
    }
}

pub struct VizierMcp(Arc<Box<dyn VizierMcpClient + Send + Sync + 'static>>);

#[async_trait::async_trait]
impl VizierMcpClient for VizierMcp {
    async fn tools(&self) -> Result<Vec<ToolDefinition>> {
        self.0.tools().await
    }

    async fn call(
        &self,
        tool_name: String,
        args: serde_json::Map<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        self.0.call(tool_name, args).await
    }
}

#[async_trait::async_trait]
pub trait VizierMcpClient {
    async fn tools(&self) -> Result<Vec<ToolDefinition>>;
    async fn call(
        &self,
        tool_name: String,
        args: serde_json::Map<String, serde_json::Value>,
    ) -> Result<serde_json::Value>;
}

#[async_trait::async_trait]
impl<S: Service<RoleClient>> VizierMcpClient for RunningService<RoleClient, S> {
    async fn tools(&self) -> Result<Vec<ToolDefinition>> {
        let tools = self.list_all_tools().await?;

        Ok(tools
            .iter()
            .map(|tool| {
                let parameters = serde_json::to_value(tool.input_schema.clone()).unwrap();
                ToolDefinition {
                    name: tool.name.to_string(),
                    description: tool.description.clone().unwrap_or("".into()).to_string(),
                    parameters,
                }
            })
            .collect())
    }

    async fn call(
        &self,
        tool_name: String,
        args: serde_json::Map<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let result = self
            .call_tool(CallToolRequestParams::new(tool_name).with_arguments(args))
            .await?;

        Ok(serde_json::to_value(result.content)?)
    }
}

impl McpClientConfig {
    pub async fn to_client(&self) -> Result<VizierMcp> {
        match self {
            Self::Local { command, args, env } => {
                let command = Command::new(command).configure(|cmd| {
                    cmd.args(args).envs(env.clone().unwrap_or(HashMap::new()));
                });
                let transport = TokioChildProcess::new(command)?;

                let client = (().serve(transport)).await?;
                Ok(VizierMcp(Arc::new(Box::new(client))))
            }
            Self::Http { uri } => {
                let transport = StreamableHttpClientTransport::from_uri(uri.clone());
                let client_info = ClientInfo::new(
                    ClientCapabilities::default(),
                    Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
                );

                let client = client_info.serve(transport).await?;
                Ok(VizierMcp(Arc::new(Box::new(client))))
            }
        }
    }
}
