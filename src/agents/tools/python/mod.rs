//! The Python sandbox tools: `execute_python` and, in code mode, the two
//! documentation tools. The engine itself lives in `crate::sandbox`.

use std::sync::Arc;

use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::Instrument;

use crate::{
    agents::tools::{ToolContext, ToolRouter, VizierTool},
    error::VizierError,
    sandbox::{self, NoToolsBridge, SandboxBridge, SandboxLimits},
    schema::{VizierResponse, VizierResponseContent},
};

mod bridge;
mod docs_tools;

use bridge::RouterBridge;
pub use docs_tools::{DescribeToolFunction, ListToolFunctions};

pub struct ExecutePython {
    router: ToolRouter,
    limits: SandboxLimits,
    description: String,
}

impl ExecutePython {
    pub fn new(router: ToolRouter, limits: SandboxLimits, tools_timeout: &str) -> Self {
        Self {
            router,
            limits,
            description: execute_python_description(limits.tools_enabled, tools_timeout),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ExecutePythonInput {
    #[schemars(
        description = "Python source to run. The value of the last expression is returned as `result`."
    )]
    pub code: String,
}

#[async_trait::async_trait]
impl VizierTool for ExecutePython {
    type Input = ExecutePythonInput;
    type Output = VizierResponse;

    fn name() -> String {
        "execute_python".into()
    }

    fn description(&self) -> String {
        self.description.clone()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let span = tracing::info_span!(
            "python_exec",
            agent_id = %ctx.session.0,
            session = %ctx.session.to_slug(),
            duration_ms = tracing::field::Empty,
        );

        let bridge = self
            .limits
            .tools_enabled
            .then(|| Arc::new(RouterBridge::new(self.router.clone(), ctx.clone(), self.limits.timeout)));
        let engine_bridge: Arc<dyn SandboxBridge> = match &bridge {
            Some(bridge) => bridge.clone(),
            None => Arc::new(NoToolsBridge),
        };

        let mut report = sandbox::execute(&args.code, self.limits, engine_bridge)
            .instrument(span.clone())
            .await;

        let attachments = match bridge {
            Some(bridge) => {
                let (tool_calls, attachments) = bridge.take_parts();
                report.tool_calls = tool_calls;
                attachments
            }
            None => vec![],
        };

        span.in_scope(|| match &report.error {
            None => tracing::info!(ok = true, duration_ms = report.duration_ms, "python script finished"),
            Some(err) if err.limit.is_some() => tracing::warn!(
                ok = false,
                error.kind = ?err.kind,
                limit = err.limit.as_deref().unwrap_or_default(),
                duration_ms = report.duration_ms,
                "python script hit a limit"
            ),
            Some(err) => tracing::info!(
                ok = false,
                error.kind = ?err.kind,
                duration_ms = report.duration_ms,
                "python script failed"
            ),
        });

        Ok(VizierResponse {
            timestamp: Utc::now(),
            content: VizierResponseContent::ToolResponse {
                response: serde_json::to_value(report)
                    .map_err(|err| VizierError(err.to_string()))?,
            },
            attachments,
        })
    }
}

/// The `execute_python` description, with the agent's tool timeout interpolated.
pub fn execute_python_description(code_mode: bool, tools_timeout: &str) -> String {
    let access = if code_mode {
        format!(
            "The sandbox has NO filesystem, network, environment or OS access. This agent's tools ARE callable
from the script as plain functions with keyword arguments, e.g.
    hits = memory_read(query=\"rust releases\")
    page = fetch_webpage(url=hits[0][\"url\"])
Call `list_tool_functions` to see every available function and `describe_tool_function` for a
function's parameters, return shape and example — or call `list_tools()` / `describe_tool(\"name\")`
from inside the script. A tool error is raised as an exception you may catch; an uncaught one ends
the run. The whole script, including every tool call it makes, must finish within {tools_timeout}.
Each run is stateless; loop and aggregate inside one script and return only what you need."
        )
    } else {
        "The sandbox has NO filesystem, network, environment or OS access, and this agent's tools are
NOT callable from scripts (calling one raises NameError). Each run is stateless."
            .to_string()
    };

    format!(
        "Run a Python script in an isolated sandbox and get back the value of its last expression
(`result`) plus anything it printed (`stdout`). Use it for exact computation: arithmetic,
date/time math, parsing, sorting, de-duplication, regex, JSON transformation, small algorithms.

{access}

Supported: functions, closures, lambdas, simple classes (no inheritance), dataclasses,
comprehensions, try/except/finally, with, f-strings, and these modules:
json, math, datetime, re, collections, itertools, functools, dataclasses, typing, base64, copy,
random, unicodedata.
Not supported: generators/yield, match, del, inheritance, @property/@classmethod, user-defined
exception classes, eval/exec, third-party packages, time, hashlib, io, socket, subprocess.

Limits: {tools_timeout} wall-clock (the same tool timeout as every other tool), recursion depth 1000,
script size 64 KiB, any single allocation over 1 GiB (e.g. \"a\" * 10**10) raises MemoryError, and
print() output up to 10 MiB. Exceeding a limit ends the run with an error naming the limit.
Return plain data (str, int, float, bool, None, list, dict); other objects cannot be returned.
Keep results small: everything you return or print is delivered to you verbatim.
On error you get the exception and a traceback with line numbers — fix the script and re-run."
    )
}
