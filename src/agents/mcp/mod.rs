//! One agent's MCP servers (#53).
//!
//! A server that crashes, restarts or stops answering must cost the agent that server's tools
//! and nothing else. Each `McpServer` keeps its connection and its tool list, lists tools only
//! when the cached list is old or the server said it changed, bounds every connect and listing
//! by a timeout, and when the connection drops, rebuilds it from config in the background,
//! with backoff.

use std::{
    collections::HashMap,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::Result;
use chrono::Utc;
use rig_core::completion::ToolDefinition;
use tokio::{process::Command, sync::Mutex, time::Instant};

use rmcp::{
    ClientHandler, RoleClient, ServiceError, ServiceExt,
    model::{CallToolRequestParams, ClientCapabilities, ClientConfig, ContentBlock, Implementation},
    service::{NotificationContext, RunningService},
    transport::{ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess},
};

use crate::{
    config::tools::mcp::McpClientConfig,
    schema::{VizierAttachment, VizierAttachmentContent, VizierResponse, VizierResponseContent},
};

/// Long enough for a stdio server started through `npx`/`uvx` to fetch its package.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(60);
/// A turn can't start until every server has listed its tools or given up.
const LIST_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a tool list is trusted without the server saying it changed.
const TOOLS_TTL: Duration = Duration::from_secs(300);
const RECONNECT_BASE: Duration = Duration::from_secs(2);
const RECONNECT_MAX: Duration = Duration::from_secs(300);

type McpClient = RunningService<RoleClient, McpHandler>;

/// Our side of the connection: identifies the client, and marks the cached tool list stale
/// when the server announces `notifications/tools/list_changed`.
#[derive(Clone)]
struct McpHandler {
    info: ClientConfig,
    tools_stale: Arc<AtomicBool>,
}

impl ClientHandler for McpHandler {
    fn get_info(&self) -> ClientConfig {
        self.info.clone()
    }

    async fn on_tool_list_changed(&self, _context: NotificationContext<RoleClient>) {
        self.tools_stale.store(true, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct ServerState {
    client: Option<Arc<McpClient>>,
    tools: Option<(Vec<ToolDefinition>, Instant)>,
    /// Failures since the last successful connect, which sets the reconnect backoff.
    failures: u32,
    retry_at: Option<Instant>,
    last_error: Option<String>,
}

pub struct McpServer {
    /// Lets `disconnect` schedule the reconnect without keeping the server alive once the
    /// agent that owns it is gone.
    me: Weak<Self>,
    name: String,
    config: McpClientConfig,
    state: Mutex<ServerState>,
    tools_stale: Arc<AtomicBool>,
}

impl McpServer {
    /// A server that is not connected yet: it connects on `warm_up` or its first call.
    pub fn new(name: String, config: McpClientConfig) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            me: me.clone(),
            name,
            config,
            state: Mutex::new(ServerState::default()),
            tools_stale: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Connect and list tools in the background, so no turn waits on a connect.
    pub fn warm_up(self: &Arc<Self>) {
        let server = self.clone();
        tokio::spawn(async move {
            let mut state = server.state.lock().await;
            if server.connect(&mut state).await.is_some() {
                server.list(&mut state).await;
            }
        });
    }

    /// The server's tools, or none while it is unreachable. Never fails, and never holds a
    /// turn longer than `LIST_TIMEOUT` waiting for the server plus one listing: a server
    /// that isn't connected is reconnected in the background and its tools return on a later
    /// turn.
    pub async fn tools(self: &Arc<Self>) -> Vec<ToolDefinition> {
        let Ok(mut state) = tokio::time::timeout(LIST_TIMEOUT, self.state.lock()).await else {
            tracing::warn!(server = %self.name, "MCP server busy connecting, its tools are left out of this turn");
            return vec![];
        };

        if self.live_client(&mut state).is_none() {
            if !state.retry_at.is_some_and(|at| Instant::now() < at) {
                self.warm_up();
            }
            return vec![];
        }

        let fresh = state
            .tools
            .as_ref()
            .is_some_and(|(_, listed_at)| listed_at.elapsed() < TOOLS_TTL)
            && !self.tools_stale.load(Ordering::SeqCst);
        if fresh {
            return state
                .tools
                .as_ref()
                .map(|(t, _)| t.clone())
                .unwrap_or_default();
        }
        self.list(&mut state).await
    }

    /// List the connected server's tools into the cache.
    async fn list(&self, state: &mut ServerState) -> Vec<ToolDefinition> {
        let Some(client) = state.client.clone() else {
            return vec![];
        };

        self.tools_stale.store(false, Ordering::SeqCst);
        match tokio::time::timeout(LIST_TIMEOUT, client.list_all_tools()).await {
            Ok(Ok(tools)) => {
                let defs: Vec<ToolDefinition> = tools
                    .iter()
                    .filter_map(|tool| {
                        Some(ToolDefinition {
                            name: tool.name.to_string(),
                            description: tool.description.clone().unwrap_or_default().to_string(),
                            parameters: serde_json::to_value(tool.input_schema.clone()).ok()?,
                        })
                    })
                    .collect();
                if state.failures > 0 {
                    tracing::info!(server = %self.name, "MCP server reconnected");
                }
                state.tools = Some((defs.clone(), Instant::now()));
                state.failures = 0;
                state.last_error = None;
                defs
            }
            // The server answered with an error: it is up, so keep what it listed before.
            Ok(Err(ServiceError::McpError(e))) => {
                tracing::warn!(server = %self.name, "MCP server failed to list tools: {e}");
                state
                    .tools
                    .as_ref()
                    .map(|(t, _)| t.clone())
                    .unwrap_or_default()
            }
            Ok(Err(e)) => {
                self.disconnect(state, format!("listing tools failed: {e}"));
                vec![]
            }
            // A hung server is dropped, which for a stdio server ends its process, and is
            // started afresh on a later use.
            Err(_) => {
                self.disconnect(
                    state,
                    format!("no answer to tools/list within {LIST_TIMEOUT:?}"),
                );
                vec![]
            }
        }
    }

    pub async fn call(
        &self,
        tool_name: String,
        args: serde_json::Map<String, serde_json::Value>,
    ) -> Result<VizierResponse> {
        let client = {
            let mut state = self.state.lock().await;
            match self.connect(&mut state).await {
                Some(client) => client,
                None => anyhow::bail!(
                    "MCP server '{}' is unavailable: {}",
                    self.name,
                    state.last_error.as_deref().unwrap_or("not connected")
                ),
            }
        };

        let result = match client
            .call_tool(CallToolRequestParams::new(tool_name).with_arguments(args))
            .await
        {
            Ok(result) => result,
            Err(e) => {
                if is_connection_error(&e) {
                    let mut state = self.state.lock().await;
                    // Only if nobody has reconnected in the meantime.
                    if state
                        .client
                        .as_ref()
                        .is_some_and(|c| Arc::ptr_eq(c, &client))
                    {
                        self.disconnect(&mut state, format!("tool call failed: {e}"));
                    }
                }
                return Err(e.into());
            }
        };

        let mut contents = vec![];
        let mut attachments = vec![];
        for content in result.content {
            match content {
                ContentBlock::Text(text) => {
                    contents.push(serde_json::Value::String(text.text));
                }
                ContentBlock::Image(image) => {
                    let ext = image.mime_type.replace("image/", "");
                    attachments.push(VizierAttachment {
                        filename: format!("{}.{}", uuid::Uuid::new_v4(), ext),
                        content: VizierAttachmentContent::Base64(image.data),
                    });
                }
                // Audio, embedded resources and resource links: handed to the model as the
                // server sent them rather than failing the call.
                other => contents.push(serde_json::to_value(&other)?),
            }
        }

        Ok(VizierResponse {
            timestamp: Utc::now(),
            content: VizierResponseContent::ToolResponse {
                response: serde_json::Value::Array(contents),
            },
            attachments,
            ..Default::default()
        })
    }

    /// The current client if its connection is still open; a closed one is dropped and
    /// counted as a failure.
    fn live_client(&self, state: &mut ServerState) -> Option<Arc<McpClient>> {
        match &state.client {
            Some(client) if !client.is_closed() && !client.is_transport_closed() => {
                Some(client.clone())
            }
            Some(_) => {
                self.disconnect(state, "connection closed".into());
                None
            }
            None => None,
        }
    }

    /// A live client, connecting if there is none and the backoff allows another attempt.
    async fn connect(&self, state: &mut ServerState) -> Option<Arc<McpClient>> {
        if let Some(client) = self.live_client(state) {
            return Some(client);
        }
        if state.retry_at.is_some_and(|at| Instant::now() < at) {
            return None;
        }

        let handler = McpHandler {
            info: ClientConfig::new(
                ClientCapabilities::default(),
                Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
            ),
            tools_stale: self.tools_stale.clone(),
        };
        match tokio::time::timeout(CONNECT_TIMEOUT, self.config.connect(handler)).await {
            // The failure count stays until the server lists its tools, so one that connects
            // but never answers still backs off.
            Ok(Ok(client)) => {
                let client = Arc::new(client);
                state.client = Some(client.clone());
                state.tools = None;
                state.retry_at = None;
                Some(client)
            }
            Ok(Err(e)) => {
                self.disconnect(state, format!("connect failed: {e}"));
                None
            }
            Err(_) => {
                self.disconnect(
                    state,
                    format!("connect timed out after {CONNECT_TIMEOUT:?}"),
                );
                None
            }
        }
    }

    /// Drop the connection and schedule the next attempt.
    fn disconnect(&self, state: &mut ServerState, reason: String) {
        state.client = None;
        state.tools = None;
        state.failures += 1;
        let wait = RECONNECT_BASE
            .saturating_mul(1u32 << (state.failures - 1).min(16))
            .min(RECONNECT_MAX);
        state.retry_at = Some(Instant::now() + wait);
        tracing::warn!(
            server = %self.name,
            failures = state.failures,
            retry_in_secs = wait.as_secs(),
            "MCP server unavailable, its tools are left out until it reconnects: {reason}"
        );
        state.last_error = Some(reason);

        let me = self.me.clone();
        tokio::spawn(async move {
            tokio::time::sleep(wait).await;
            if let Some(server) = me.upgrade() {
                server.warm_up();
            }
        });
    }
}

/// Errors that mean the connection itself is gone, as opposed to the server answering a call
/// with an error.
fn is_connection_error(e: &ServiceError) -> bool {
    matches!(
        e,
        ServiceError::TransportSend(_)
            | ServiceError::TransportClosed
            | ServiceError::Cancelled { .. }
    )
}

impl McpClientConfig {
    async fn connect(&self, handler: McpHandler) -> Result<McpClient> {
        match self {
            Self::Local { command, args, env } => {
                let command = Command::new(command).configure(|cmd| {
                    cmd.args(args).envs(env.clone().unwrap_or(HashMap::new()));
                });
                let transport = TokioChildProcess::new(command)?;
                Ok(handler.serve(transport).await?)
            }
            Self::Http { uri } => {
                let transport = StreamableHttpClientTransport::from_uri(uri.clone());
                Ok(handler.serve(transport).await?)
            }
        }
    }
}
