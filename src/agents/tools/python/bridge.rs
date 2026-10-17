//! Connects scripts to the agent's tools: every nested call goes through the same
//! `ToolRouter` dispatch, session hooks and per-tool timeout as a direct call.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use tokio::sync::OnceCell;

use crate::{
    agents::{
        hook::VizierSessionHook,
        tools::{CatalogueEntry, ToolContext, ToolRouter, ToolTarget, turn_catalogue},
    },
    agents::tools::python::docs_tools::entry_doc,
    sandbox::{BridgeError, SandboxBridge, ToolFunctionDoc, ToolInvocationRecord, docs},
    schema::{VizierAttachment, VizierResponseContent},
};

pub struct RouterBridge {
    router: ToolRouter,
    ctx: ToolContext,
    tool_timeout: Duration,
    /// The turn's catalogue (the router's, then the client's), snapshotted once per run so
    /// names stay put while it runs.
    catalogue: OnceCell<Vec<CatalogueEntry>>,
    invocations: Mutex<Vec<ToolInvocationRecord>>,
    attachments: Mutex<Vec<VizierAttachment>>,
}

impl RouterBridge {
    pub fn new(router: ToolRouter, ctx: ToolContext, tool_timeout: Duration) -> Self {
        Self {
            router,
            ctx,
            tool_timeout,
            catalogue: OnceCell::new(),
            invocations: Mutex::new(vec![]),
            attachments: Mutex::new(vec![]),
        }
    }

    /// The invocation records and nested attachments collected during the run.
    pub fn take_parts(&self) -> (Vec<ToolInvocationRecord>, Vec<VizierAttachment>) {
        (take(&self.invocations), take(&self.attachments))
    }

    async fn entries(&self) -> &[CatalogueEntry] {
        self.catalogue
            .get_or_init(|| turn_catalogue(&self.router, &self.ctx))
            .await
    }

    async fn entry(&self, function: &str) -> Option<&CatalogueEntry> {
        self.entries().await.iter().find(|entry| entry.name == function)
    }

    /// One nested call, exactly as the agent loop makes a direct one. A client tool goes to
    /// the client instead of the router, inside the same hooks and timeout.
    /// The call id is a client call's history uid, `None` for every other call.
    async fn dispatch(&self, tool: String, arguments: String) -> (Option<String>, Result<serde_json::Value, String>) {
        let client = match self.entry(&tool).await.map(|entry| &entry.target) {
            Some(ToolTarget::Client { .. }) => self.ctx.client.clone(),
            _ => None,
        };
        let call_id = std::sync::Mutex::new(None);
        let result = self.dispatch_to(tool, arguments, client, &call_id).await;
        (call_id.into_inner().unwrap_or_default(), result)
    }

    async fn dispatch_to(
        &self,
        tool: String,
        arguments: String,
        client: Option<std::sync::Arc<crate::agents::client::ClientTools>>,
        call_id: &std::sync::Mutex<Option<String>>,
    ) -> Result<serde_json::Value, String> {
        let (mut tool, mut arguments) = (tool, arguments);
        if let Some(hooks) = &self.ctx.hooks {
            (tool, arguments) = hooks
                .on_nested_tool_call(tool, arguments)
                .await
                .map_err(|err| err.to_string())?;
        }

        let call = async {
            match &client {
                Some(client) => {
                    let arguments = serde_json::from_str(&arguments).map_err(anyhow::Error::from)?;
                    let (id, result) = client.call(&tool, arguments).await;
                    if let Ok(mut slot) = call_id.lock() {
                        *slot = id;
                    }
                    result
                }
                None => self.router.call(tool.clone(), arguments, &self.ctx).await,
            }
        };
        let mut response = tokio::time::timeout(self.tool_timeout, call)
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
        self.entries()
            .await
            .iter()
            .map(|entry| {
                docs::describe_as(
                    entry.name.clone(),
                    &entry.definition,
                    None,
                    Some(entry.category.clone()),
                )
            })
            .collect()
    }

    async fn describe(&self, function: &str) -> Option<ToolFunctionDoc> {
        Some(entry_doc(&self.router, &self.ctx, self.entry(function).await?))
    }

    async fn call(
        &self,
        function: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, BridgeError> {
        // The catalogue name is the one `ToolRouter::call` dispatches on.
        let entry = self.entry(function).await.ok_or(BridgeError::UnknownFunction)?;
        let tool = entry.name.clone();
        let is_client = matches!(entry.target, ToolTarget::Client { .. });

        let started = Instant::now();
        let (call_id, result) = self.dispatch(tool.clone(), arguments.to_string()).await;
        let duration_ms = started.elapsed().as_millis() as u64;

        // A client call's arguments are already stored in full on its `ClientToolCall` entry
        // (a page can be 256 KB), so the report keeps a reference to it instead.
        let arguments = match is_client {
            true => serde_json::json!({ "client_call": call_id }),
            false => arguments,
        };

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
