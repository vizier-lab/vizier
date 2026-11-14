//! The agent's view of its background work (spec 058 contracts/detached-flag.md §4):
//! `list_detached_scripts`, `get_detached_script` and `cancel_detached_script`. Each acts on
//! the calling conversation's scripts only, and answers a refusal as a `status` result.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::{
        detached::DetachedScripts,
        tools::{ToolCategory, ToolContext, VizierTool},
    },
    error::VizierError,
    schema::{Canceller, DetachedScript},
    storage::{
        VizierStorage,
        detached_script::{CancelOutcome, DetachedScriptStorage},
    },
};

use std::sync::Arc;

/// How many final scripts `list_detached_scripts` shows beside the running ones.
const LISTED_FINAL: usize = 50;

fn read_err(e: anyhow::Error) -> VizierError {
    VizierError(format!("could not read background work: {e}"))
}

/// The script, if it belongs to this conversation.
async fn own(
    storage: &VizierStorage,
    ctx: &ToolContext,
    id: &str,
) -> Result<Option<DetachedScript>, VizierError> {
    Ok(storage
        .get_detached_script(id.trim())
        .await
        .map_err(read_err)?
        .filter(|script| script.session == ctx.session))
}

fn not_found() -> serde_json::Value {
    serde_json::json!({ "status": "not_found" })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScriptFilter {
    Running,
    Done,
    Stopped,
    #[default]
    All,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListDetachedScriptsArgs {
    #[schemars(description = "running, done (finished on its own), stopped (cancelled, interrupted or timed out) or all (default)")]
    #[serde(default)]
    pub state: ScriptFilter,
}

pub struct ListDetachedScripts {
    pub storage: Arc<VizierStorage>,
}

#[async_trait::async_trait]
impl VizierTool for ListDetachedScripts {
    type Input = ListDetachedScriptsArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Agents)
    }

    fn name() -> String {
        "list_detached_scripts".into()
    }

    fn description(&self) -> String {
        "List this conversation's background work (scripts run with execute_python detached=true), oldest first: {\"scripts\": [{id, intent, state, elapsed_secs, delivered}]}. Shows the running ones and the last 50 that ended.".into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let now = crate::time::now();
        let scripts = self
            .storage
            .list_detached_scripts(&ctx.session, LISTED_FINAL)
            .await
            .map_err(read_err)?;
        let scripts: Vec<_> = scripts
            .iter()
            .filter(|script| match args.state {
                ScriptFilter::All => true,
                ScriptFilter::Running => !script.state.is_final(),
                ScriptFilter::Stopped => script.state.is_stopped(),
                ScriptFilter::Done => script.state.is_final() && !script.state.is_stopped(),
            })
            .map(|script| {
                serde_json::json!({
                    "id": script.id,
                    "intent": script.intent,
                    "state": script.state,
                    "elapsed_secs": script.elapsed_secs(now),
                    "delivered": script.delivered_at.is_some(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "scripts": scripts }))
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DetachedScriptIdArgs {
    #[schemars(description = "the handle execute_python returned, e.g. d_k3x9a1b2")]
    pub id: String,
}

pub struct GetDetachedScript {
    pub storage: Arc<VizierStorage>,
}

#[async_trait::async_trait]
impl VizierTool for GetDetachedScript {
    type Input = DetachedScriptIdArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Agents)
    }

    fn name() -> String {
        "get_detached_script".into()
    }

    fn description(&self) -> String {
        "One piece of this conversation's background work: {id, intent, state, started_at, elapsed_secs, timeout_secs, parent_id, delivered, tool_calls: [{name, ok, duration_ms}], outcome?}. `outcome` (the script's report, or why it stopped) appears once it has ended. An id of another conversation is {\"status\": \"not_found\"}.".into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let Some(script) = own(&self.storage, ctx, &args.id).await? else {
            return Ok(not_found());
        };
        // The slim form: the agent wrote the script and has the arguments in its history.
        let calls: Vec<_> = script
            .tool_calls
            .as_array()
            .into_iter()
            .flatten()
            .map(|call| {
                serde_json::json!({
                    "name": call.get("name"),
                    "ok": call.get("ok"),
                    "duration_ms": call.get("duration_ms"),
                })
            })
            .collect();
        let mut out = serde_json::json!({
            "id": script.id,
            "intent": script.intent,
            "state": script.state,
            "started_at": crate::time::display(script.created_at),
            "elapsed_secs": script.elapsed_secs(crate::time::now()),
            "timeout_secs": script.timeout_secs,
            "parent_id": script.parent_id,
            "delivered": script.delivered_at.is_some(),
            "tool_calls": calls,
        });
        if script.state.is_final()
            && let Some(outcome) = &script.outcome
        {
            out["outcome"] = outcome.clone();
        }
        Ok(out)
    }
}

pub struct CancelDetachedScript {
    pub storage: Arc<VizierStorage>,
    pub scripts: DetachedScripts,
}

#[async_trait::async_trait]
impl VizierTool for CancelDetachedScript {
    type Input = DetachedScriptIdArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Agents)
    }

    fn name() -> String {
        "cancel_detached_script".into()
    }

    fn description(&self) -> String {
        "Stop a piece of this conversation's background work, what it is waiting on, and any background work it started. Nothing is delivered for it. Returns {\"status\": \"cancelled\"}, {\"status\": \"already_finished\", \"state\": …} or {\"status\": \"not_found\"}.".into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let Some(script) = own(&self.storage, ctx, &args.id).await? else {
            return Ok(not_found());
        };
        Ok(
            match self.scripts.cancel(&script.id, Canceller::Agent(ctx.session.0.clone())).await? {
                CancelOutcome::Cancelled(ids) => {
                    serde_json::json!({ "status": "cancelled", "also_cancelled": &ids[1..] })
                }
                CancelOutcome::AlreadyFinished(state) => {
                    serde_json::json!({ "status": "already_finished", "state": state })
                }
                CancelOutcome::NotFound => not_found(),
            },
        )
    }
}
