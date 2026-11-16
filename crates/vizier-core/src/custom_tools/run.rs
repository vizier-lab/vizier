//! Calling a custom tool (`specs/061-agent-custom-tools` R2, contracts/agent-tools.md §3): the
//! saved script runs through the same `run_script` an `execute_python` call uses, with the
//! validated arguments bound as `input`, under the calling turn's context, router and tool
//! timeout. A failure is an error whose text names the tool, the failing step and the calls made.

use tracing::Instrument;

use super::{CustomTools, schema};
use crate::{
    agents::tools::{ToolContext, ToolRouter, turn_source::BridgeEnv},
    sandbox::{ExecutionReport, SandboxLimits, ToolInvocationRecord},
    schema::{VizierAttachment, VizierResponse, VizierResponseContent},
    storage::custom_tool::{CustomToolDefinition, CustomToolStorage},
};

/// Run `definition`'s script once with `input` (already validated), as `ctx`'s turn would. Used by
/// a call and by the sample run before a save.
pub async fn run_definition(
    router: &ToolRouter,
    ctx: &ToolContext,
    definition: &CustomToolDefinition,
    input: serde_json::Value,
) -> (ExecutionReport, Vec<VizierAttachment>) {
    run_with_timeout(router, ctx, definition, input, tool_timeout(ctx)).await
}

async fn run_with_timeout(
    router: &ToolRouter,
    ctx: &ToolContext,
    definition: &CustomToolDefinition,
    input: serde_json::Value,
    timeout: std::time::Duration,
) -> (ExecutionReport, Vec<VizierAttachment>) {
    let mut nested = ctx.clone();
    nested.custom.chain.push(definition.name.clone());
    let limits = SandboxLimits { timeout, tools_enabled: true };
    crate::agents::tools::python::run_script(router, &definition.script, limits, timeout, &nested, None, Some(input))
        .await
}

/// The time a sample run gets: what is left of the calling call, or a minute.
fn tool_timeout(ctx: &ToolContext) -> std::time::Duration {
    ctx.deadline
        .map(|deadline| deadline.saturating_duration_since(tokio::time::Instant::now()))
        .filter(|left| !left.is_zero())
        .unwrap_or(std::time::Duration::from_secs(60))
}

/// One call of the custom tool `function` from a script.
pub async fn call(
    tools: &CustomTools,
    function: &str,
    arguments: String,
    ctx: &ToolContext,
    env: &BridgeEnv<'_>,
) -> anyhow::Result<VizierResponse> {
    let span = crate::agents::tools::execute_tool_span(function, None);
    call_inner(tools, function, arguments, ctx, env).instrument(span).await
}

async fn call_inner(
    tools: &CustomTools,
    function: &str,
    arguments: String,
    ctx: &ToolContext,
    env: &BridgeEnv<'_>,
) -> anyhow::Result<VizierResponse> {
    let agent_id = &ctx.session.0;
    let name = function;
    let Some(tool) = tools.storage().get_custom_tool(agent_id, name).await? else {
        anyhow::bail!("{function}: you have no custom tool named {name} (was it deleted or renamed?)");
    };
    if !tool.enabled {
        let by = tool.disabled_by.as_ref().map(|by| by.label()).unwrap_or_else(|| "someone".into());
        anyhow::bail!("{function}: disabled by {by}");
    }
    if let Some(start) = ctx.custom.chain.iter().position(|running| running == name) {
        let mut path: Vec<&str> = ctx.custom.chain[start..].iter().map(String::as_str).collect();
        path.push(name);
        anyhow::bail!("{function}: cycle {}", path.join(" → "));
    }
    let arguments: serde_json::Value = match arguments.trim() {
        "" => serde_json::Value::Null,
        text => serde_json::from_str(text)?,
    };
    let definition = tool.definition();
    let input = schema::validate(&definition.input_schema, &arguments)
        .map_err(|errors| anyhow::anyhow!("{function}: invalid input: {}", errors.join("; ")))?;

    tracing::info!(agent_id = %agent_id, tool = function, version = tool.current_seq, "custom tool called");
    let (report, attachments) =
        run_with_timeout(env.router, ctx, &definition, input, env.tool_timeout).await;
    if let Ok(mut nested) = env.nested.lock() {
        *nested = report.tool_calls.clone();
    }

    let storage = tools.storage().clone();
    let tool_id = tool.id.clone();
    tokio::spawn(async move {
        if let Err(err) = storage.touch_custom_tool_called(&tool_id, crate::time::now().timestamp_millis()).await {
            tracing::debug!("could not record a custom tool call: {err}");
        }
    });

    if !report.ok {
        anyhow::bail!("{}", failure_summary(function, &report));
    }
    Ok(VizierResponse {
        timestamp: crate::time::now(),
        content: VizierResponseContent::ToolResponse { response: report.result },
        attachments,
        ..Default::default()
    })
}

/// `custom_x failed at step 3 (memory_write): …` plus the calls made, then anything printed.
pub fn failure_summary(function: &str, report: &ExecutionReport) -> String {
    let message = report.error.as_ref().map(|error| error.message.clone()).unwrap_or_default();
    let failed = report.tool_calls.iter().find(|call| !call.ok);
    let mut text = match failed {
        Some(call) => format!(
            "{function} failed at step {} ({}): {}",
            call.seq,
            call.name,
            call.error.clone().unwrap_or(message)
        ),
        None => format!("{function} failed: {message}"),
    };
    if !report.tool_calls.is_empty() {
        let steps: Vec<String> = report.tool_calls.iter().map(step).collect();
        text.push_str(&format!("\ncalls made: {}", steps.join(", ")));
    }
    let printed = report.stdout.trim();
    if !printed.is_empty() {
        let shown: String = printed.chars().take(2_000).collect();
        text.push_str(&format!("\nprinted: {shown}"));
    }
    text
}

fn step(call: &ToolInvocationRecord) -> String {
    format!("{} {} {}", call.seq, call.name, if call.ok { "ok" } else { "failed" })
}
