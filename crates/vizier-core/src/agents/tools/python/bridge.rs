//! Connects scripts to the agent's tools: every nested call goes through the same
//! `ToolRouter` dispatch, session hooks and per-tool timeout as a direct call.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use tokio::sync::{OnceCell, watch};

use crate::{
    agents::{
        hook::VizierSessionHook,
        tools::{
            CatalogueEntry, ToolContext, ToolRouter, ToolTarget, VizierToolDyn,
            retired_hint, tool_output_to_response, turn_catalogue,
        },
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
    /// Signals an in-flight nested call to stop when the sandbox is dropped.
    cancelled: watch::Sender<bool>,
    /// Told the records so far after every nested call (a detached script's progress).
    progress: Option<super::ProgressFn>,
}

impl RouterBridge {
    pub fn new(router: ToolRouter, ctx: ToolContext, tool_timeout: Duration) -> Self {
        let (cancelled, _) = watch::channel(false);
        Self {
            router,
            ctx,
            tool_timeout,
            catalogue: OnceCell::new(),
            invocations: Mutex::new(vec![]),
            attachments: Mutex::new(vec![]),
            cancelled,
            progress: None,
        }
    }

    /// Report each finished nested call to `progress` (spec 058: a detached script's panel).
    pub fn with_progress(mut self, progress: super::ProgressFn) -> Self {
        self.progress = Some(progress);
        self
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
        let target = self.entry(&tool).await.map(|entry| entry.target.clone());
        let call_id = std::sync::Mutex::new(None);
        let result = self.dispatch_to(tool, arguments, target, &call_id).await;
        (call_id.into_inner().unwrap_or_default(), result)
    }

    async fn dispatch_to(
        &self,
        tool: String,
        arguments: String,
        target: Option<ToolTarget>,
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
            match &target {
                Some(ToolTarget::Client { .. }) => {
                    let Some(client) = self.ctx.client.clone() else {
                        return Err(anyhow::anyhow!("{tool} is no longer offered by this client"));
                    };
                    let arguments = serde_json::from_str(&arguments).map_err(anyhow::Error::from)?;
                    let (id, result) = client.call(&tool, arguments, Some(&self.ctx.secret_redactor)).await;
                    if let Ok(mut slot) = call_id.lock() {
                        *slot = id;
                    }
                    result
                }
                // Project coding tools live only in this turn's catalogue, not in the
                // router's; the registry re-checks what this turn may use (contract §1).
                Some(ToolTarget::Project) => {
                    let value = crate::projects::tools::call(&tool, arguments, &self.ctx).await?;
                    Ok(crate::schema::VizierResponse {
                        timestamp: crate::time::now(),
                        content: VizierResponseContent::ToolResponse { response: value },
                        attachments: vec![],
                        ..Default::default()
                    })
                }
                _ => self.router.call(tool.clone(), arguments, &self.ctx).await,
            }
        };
        // A cancel (the sandbox dropped, or a detached script cancelled) stops whatever call
        // is in flight, whichever tool it is: dropping the call's future runs its own drop
        // guards (a shell call kills its process). Workers a script spawned keep running.
        let mut cancelled = self.cancelled.subscribe();
        if *cancelled.borrow() {
            return Err("tool call cancelled".to_string());
        }
        let timed = tokio::time::timeout(self.tool_timeout, call);
        let result = tokio::select! {
            result = timed => result,
            _ = cancelled.changed() => return Err("tool call cancelled".to_string()),
        };
        let mut response = result
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
    fn cancel(&self) {
        self.cancelled.send_replace(true);
    }

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
        let entry = match self.entry(function).await {
            Some(entry) => entry,
            None => {
                return Err(match retired_hint(function) {
                    Some(hint) => BridgeError::Tool(hint.into()),
                    None => BridgeError::UnknownFunction,
                });
            }
        };
        let tool = entry.name.clone();
        let is_client = matches!(entry.target, ToolTarget::Client { .. });
        let flagged = entry.ends_turn;

        let started = Instant::now();
        let (call_id, result) = self.dispatch(tool.clone(), arguments.to_string()).await;
        let duration_ms = started.elapsed().as_millis() as u64;
        let ends_turn = flagged && result.as_ref().is_ok_and(|value| !is_deferred(value));

        // A client call's arguments are already stored in full on its `ClientToolCall` entry
        // (a page can be 256 KB), so the report keeps a reference to it instead.
        let mut arguments = match is_client {
            true => serde_json::json!({ "client_call": call_id }),
            false => arguments,
        };

        // The script keeps the real result and error; every copy that leaves the turn is
        // scrubbed first (`specs/027-agent-secrets` research R4).
        self.ctx.secret_redactor.json(&mut arguments);
        let logged_error = result
            .as_ref()
            .err()
            .map(|err| self.ctx.secret_redactor.text(err));

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
                error: logged_error,
                duration_ms,
                ends_turn,
            });
            if let Some(progress) = &self.progress {
                progress(&invocations);
            }
        }

        result.map_err(|err| BridgeError::Tool(self.ctx.secret_redactor.text(&err)))
    }
}

/// A client call that was handed on for later rather than answered: not a result, so it
/// never ends the turn.
fn is_deferred(value: &serde_json::Value) -> bool {
    value.get("status").and_then(serde_json::Value::as_str) == Some("deferred")
}

fn take<T>(list: &Mutex<Vec<T>>) -> Vec<T> {
    list.lock().map(|mut items| std::mem::take(&mut *items)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::{
        agents::tools::{ToolCategory, ToolContext, VizierTool, VizierToolSet},
        error::VizierError,
        schema::{VizierChannelId, VizierSession},
    };

    const TOKEN: &str = "ghp_nested-secret-value-99";

    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    struct ProbeInput {
        token: String,
        fail: bool,
    }

    /// A tool that does what a real one does with a credential: sends it on, and when the far end
    /// refuses, repeats it back in its error.
    struct Probe;

    #[async_trait::async_trait]
    impl VizierTool for Probe {
        type Input = ProbeInput;
        type Output = String;

        fn category() -> Option<ToolCategory> {
            Some(ToolCategory::Secrets)
        }

        fn name() -> String {
            "probe_credential".into()
        }

        fn description(&self) -> String {
            "Test probe.".into()
        }

        async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
            if args.fail {
                Err(VizierError(format!("401: credential {} was refused", args.token)))
            } else {
                Ok(format!("accepted {}", args.token))
            }
        }
    }

    fn bridge() -> RouterBridge {
        let router = ToolRouter {
            default_toolset: VizierToolSet::new().tool(Probe),
            user_toolset: VizierToolSet::new(),
            integration_toolset: VizierToolSet::new(),
            addons: Default::default(),
            mcp: Default::default(),
            reserved: Arc::new(vec![]),
        };
        let ctx = ToolContext {
            session: VizierSession(
                "agent-1".into(),
                VizierChannelId::HTTP("alice".into(), "webui".into()),
                Some("t".into()),
            ),
            pending_attachments: Default::default(),
            hooks: None,
            background_depth: 0,
            subagent_depth: 0,
            loaded_images: Default::default(),
            deadline: None,
            client: None,
            questions: crate::questions::QuestionsHandle::new(Default::default()),
            interactive: true,
            secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
            dream_sources: vec![],
            project: None,
            detached: false,
            parent_detached_id: None,
        };
        ctx.secret_redactor.arm(TOKEN);
        RouterBridge::new(router, ctx, Duration::from_secs(5))
    }

    /// A value handed to a nested tool call reaches the tool whole, and leaves nowhere else:
    /// not in the recorded arguments, the recorded error, or the error the script catches
    /// (FR-042). The script keeps the real result, because it is the one holding the value.
    #[tokio::test]
    async fn a_value_passed_to_a_nested_call_is_scrubbed_from_every_copy() {
        let bridge = bridge();

        let failed = bridge
            .call("probe_credential", serde_json::json!({ "token": TOKEN, "fail": true }))
            .await
            .expect_err("the probe fails");
        let BridgeError::Tool(message) = failed else { panic!("not a tool error: {failed:?}") };
        assert!(!message.contains(TOKEN), "the caught error still carries the value: {message}");
        assert!(message.contains("[redacted]"), "{message}");

        let ok = bridge
            .call("probe_credential", serde_json::json!({ "token": TOKEN, "fail": false }))
            .await
            .expect("the probe succeeds");
        assert!(
            ok.to_string().contains(TOKEN),
            "the script itself must receive the real result, byte for byte: {ok}"
        );

        let (records, _) = bridge.take_parts();
        assert_eq!(records.len(), 2);
        let recorded = serde_json::to_string(&records).unwrap();
        assert!(!recorded.contains(TOKEN), "a recorded call still carries the value: {recorded}");
        assert!(recorded.contains("[redacted]"));
    }

    /// A tool that ends the turn, like `ui_render` or `propose_plan`.
    struct Finish;

    #[async_trait::async_trait]
    impl VizierTool for Finish {
        type Input = serde_json::Value;
        type Output = String;
        fn category() -> Option<ToolCategory> {
            Some(ToolCategory::Webui)
        }
        fn name() -> String {
            "finish_with_page".into()
        }
        fn ends_turn() -> bool {
            true
        }
        fn description(&self) -> String {
            "Test.".into()
        }
        async fn call(&self, _: Self::Input, _: &ToolContext) -> Result<Self::Output, VizierError> {
            Ok("shown".into())
        }
    }

    /// A tool that needs a person to answer, like `ask_question` and `ask_sensitive_input`.
    struct Ask;

    #[async_trait::async_trait]
    impl VizierTool for Ask {
        type Input = serde_json::Value;
        type Output = String;
        fn category() -> Option<ToolCategory> {
            Some(ToolCategory::Questions)
        }
        fn name() -> String {
            "ask_the_person".into()
        }
        fn available(&self, ctx: &ToolContext) -> bool {
            ctx.interactive
        }
        fn description(&self) -> String {
            "Test.".into()
        }
        async fn call(&self, _: Self::Input, _: &ToolContext) -> Result<Self::Output, VizierError> {
            Ok("answered".into())
        }
    }

    /// Spec 058 contracts/detached-flag.md §2: inside a detached script the live-turn tools are
    /// gone from the catalogue and from `list_tools()`, and calling one is an unknown function.
    #[tokio::test]
    async fn a_detached_script_sees_no_live_turn_tools() {
        let router = ToolRouter {
            default_toolset: VizierToolSet::new().tool(Probe).tool(Finish).tool(Ask),
            user_toolset: VizierToolSet::new(),
            integration_toolset: VizierToolSet::new(),
            addons: Default::default(),
            mcp: Default::default(),
            reserved: Arc::new(vec![]),
        };
        let inline = bridge().ctx;
        let names = |entries: Vec<CatalogueEntry>| -> Vec<String> {
            entries.into_iter().map(|e| e.name).collect()
        };
        let all = names(turn_catalogue(&router, &inline).await);
        assert!(all.contains(&"finish_with_page".to_string()) && all.contains(&"ask_the_person".to_string()), "{all:?}");

        let mut detached = inline.clone();
        detached.detached = true;
        detached.interactive = false;
        detached.client = None;
        let seen = names(turn_catalogue(&router, &detached).await);
        assert_eq!(seen, vec!["probe_credential".to_string()]);

        let bridge = RouterBridge::new(router, detached, Duration::from_secs(5));
        let listed: Vec<String> = bridge.catalogue().await.into_iter().map(|d| d.name).collect();
        assert_eq!(listed, seen);
        assert!(matches!(
            bridge.call("ask_the_person", serde_json::json!({})).await,
            Err(BridgeError::UnknownFunction)
        ));
        assert!(matches!(
            bridge.call("finish_with_page", serde_json::json!({})).await,
            Err(BridgeError::UnknownFunction)
        ));
    }
}
