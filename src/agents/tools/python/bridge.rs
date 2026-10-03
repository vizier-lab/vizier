//! Connects scripts to the agent's tools: every nested call goes through the same
//! `ToolRouter` dispatch, session hooks and per-tool timeout as a direct call.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use rig_core::completion::ToolDefinition;
use tokio::sync::OnceCell;

use crate::{
    agents::{
        hook::VizierSessionHook,
        tools::{ToolContext, ToolRouter},
    },
    agents::tools::python::docs_tools::output_schema,
    sandbox::{BridgeError, SandboxBridge, ToolFunctionDoc, ToolInvocationRecord, docs},
    schema::{VizierAttachment, VizierResponseContent},
};

pub struct RouterBridge {
    router: ToolRouter,
    ctx: ToolContext,
    tool_timeout: Duration,
    /// Script function name → tool name, snapshotted once per run for names that
    /// are not native tools (MCP tools, sanitised names).
    names: OnceCell<HashMap<String, String>>,
    invocations: Mutex<Vec<ToolInvocationRecord>>,
    attachments: Mutex<Vec<VizierAttachment>>,
}

impl RouterBridge {
    pub fn new(router: ToolRouter, ctx: ToolContext, tool_timeout: Duration) -> Self {
        Self {
            router,
            ctx,
            tool_timeout,
            names: OnceCell::new(),
            invocations: Mutex::new(vec![]),
            attachments: Mutex::new(vec![]),
        }
    }

    /// The invocation records and nested attachments collected during the run.
    pub fn take_parts(&self) -> (Vec<ToolInvocationRecord>, Vec<VizierAttachment>) {
        (take(&self.invocations), take(&self.attachments))
    }

    async fn definitions(&self) -> Vec<ToolDefinition> {
        self.router.definitions().await.unwrap_or_else(|err| {
            tracing::warn!("failed to list tools for the python sandbox: {err}");
            vec![]
        })
    }

    async fn resolve(&self, function: &str) -> Option<String> {
        if self.router.default_toolset.tools.contains_key(function)
            || self.router.user_toolset.tools.contains_key(function)
        {
            return Some(function.to_string());
        }
        let names = self
            .names
            .get_or_init(|| async {
                docs::catalogue(&self.definitions().await)
                    .into_iter()
                    .map(|doc| (doc.function, doc.tool))
                    .collect()
            })
            .await;
        names.get(function).cloned()
    }


    /// One nested call, exactly as the agent loop makes a direct one.
    async fn dispatch(&self, tool: String, arguments: String) -> Result<serde_json::Value, String> {
        let (mut tool, mut arguments) = (tool, arguments);
        if let Some(hooks) = &self.ctx.hooks {
            (tool, arguments) = hooks
                .on_nested_tool_call(tool, arguments)
                .await
                .map_err(|err| err.to_string())?;
        }

        let mut response = tokio::time::timeout(
            self.tool_timeout,
            self.router.call(tool.clone(), arguments, &self.ctx),
        )
        .await
        .map_err(|_| format!("timed out after {:?}", self.tool_timeout))?
        .map_err(|err| err.to_string())?;

        if let Some(hooks) = &self.ctx.hooks {
            response = hooks
                .on_tool_response(response)
                .await
                .map_err(|err| err.to_string())?;
        }

        if let Ok(mut attachments) = self.attachments.lock() {
            attachments.append(&mut response.attachments);
        }

        Ok(match response.content {
            VizierResponseContent::ToolResponse { response } => response,
            VizierResponseContent::Message { content, .. } => serde_json::Value::String(content),
            other => serde_json::to_value(other).unwrap_or_default(),
        })
    }
}

#[async_trait::async_trait]
impl SandboxBridge for RouterBridge {
    async fn catalogue(&self) -> Vec<ToolFunctionDoc> {
        docs::catalogue(&self.definitions().await)
    }

    async fn describe(&self, function: &str) -> Option<ToolFunctionDoc> {
        let defs = self.definitions().await;
        let tool = docs::catalogue(&defs)
            .into_iter()
            .find(|doc| doc.function == function)?
            .tool;
        docs::describe_in(&defs, function, output_schema(&self.router, &tool).as_ref())
    }

    async fn call(
        &self,
        function: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, BridgeError> {
        let tool = self
            .resolve(function)
            .await
            .ok_or(BridgeError::UnknownFunction)?;

        let started = Instant::now();
        let result = self.dispatch(tool.clone(), arguments.to_string()).await;
        let duration_ms = started.elapsed().as_millis() as u64;

        tracing::info!(
            agent_id = %self.ctx.session.0,
            session = %self.ctx.session.to_slug(),
            tool = %tool,
            ok = result.is_ok(),
            duration_ms,
            // The model's copy of the report drops `arguments` (they restate the script);
            // they stay on the record here and in the invocation below.
            arguments = %arguments,
            "python script called a tool"
        );

        if let Ok(mut invocations) = self.invocations.lock() {
            let seq = invocations.len() as u32 + 1;
            invocations.push(ToolInvocationRecord {
                seq,
                name: tool,
                arguments,
                ok: result.is_ok(),
                error: result.as_ref().err().cloned(),
                duration_ms,
            });
        }

        result.map_err(BridgeError::Tool)
    }
}

fn take<T>(list: &Mutex<Vec<T>>) -> Vec<T> {
    list.lock().map(|mut items| std::mem::take(&mut *items)).unwrap_or_default()
}
