//! The Python sandbox tools: `execute_python` and the two documentation tools. The engine itself lives in `crate::sandbox`.

use std::sync::Arc;

use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::Instrument;

use crate::{
    agents::tools::{ToolCategory, ToolContext, ToolRouter, VizierTool},
    error::VizierError,
    sandbox::{self, NoToolsBridge, SandboxBridge, SandboxLimits},
    schema::{VizierResponse, VizierResponseContent},
};

mod bridge;
mod docs_tools;

use bridge::RouterBridge;
pub use docs_tools::{DescribeToolFunction, ListToolFunctions, ListToolFunctionsInput};

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
            description: execute_python_description(tools_timeout),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ExecutePythonInput {
    #[schemars(
        description = "Python source to run. The value of the last expression is returned as `result`."
    )]
    pub code: String,

    /// `String`, not `Option<String>`, on purpose: a call that omits it fails
    /// deserialization and the error goes back to the agent, which may retry. It is never
    /// passed to the sandbox — see `call`.
    #[schemars(description = "One short sentence, in plain language, saying what this \
         script is for. It is shown to the person watching in place of the code — write it \
         for them, not for yourself. E.g. \"compute the 99th percentile from the last 24h \
         of latency samples\".")]
    pub intent: String,
}

#[async_trait::async_trait]
impl VizierTool for ExecutePython {
    type Input = ExecutePythonInput;
    type Output = VizierResponse;

    fn category() -> Option<ToolCategory> {
        None
    }

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

        // `args.intent` deliberately stops here: only the code reaches the sandbox, so the
        // stated intent cannot become a script variable or affect limits, tool availability
        // or the result. It is read only by whatever renders the call, from the tool call's
        // arguments — which is also why it is not copied onto `ExecutionReport`.
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
            ..Default::default()
        })
    }
}

/// The `execute_python` description, with the agent's tool timeout interpolated.
pub fn execute_python_description(tools_timeout: &str) -> String {
    let access = format!(
            "The sandbox has NO filesystem, network, environment or OS access. This agent's tools ARE callable
from the script as plain functions with keyword arguments, e.g.
    hits = memory_search(query=\"rust releases\")
    note = memory_read(bundle=hits[0][\"bundle\"], path=hits[0][\"path\"])
`list_tool_functions(category=\"…\")` lists a category's functions (leave `category` out for all of
them) and `describe_tool_function` shows a function's parameters, return shape and example — either
as a tool, or from inside the script (`list_tools(category=…)` / `describe_tool(\"name\")` are the same
two functions under shorter names). `list_tools()` returns a list of
{{\"name\": ..., \"summary\": ..., \"category\": ...}} dicts; pass a `name` to `describe_tool`.
A tool error is raised as an exception you may catch; an uncaught one ends the run. The whole script, including every tool call it makes, must finish within {tools_timeout}.
Each run is stateless: variables do not survive to the next call, so never rebuild or re-send a
value just to keep it. Loop and aggregate inside one script and return only what you need; to
carry text between runs (e.g. a large page built in pieces) use the scratchpad tools
(`write_tmp`, `append_tmp`, `read_tmp`)."
    );

    format!(
        "Run a Python script in an isolated sandbox and get back the value of its last expression
(`result`) plus anything it printed (`stdout`). Use it for exact computation: arithmetic,
date/time math, parsing, sorting, de-duplication, regex, JSON transformation, small algorithms.

Every run needs an `intent` as well as `code`: one short sentence saying what the script is for.
The person watching sees that sentence where the script would otherwise be, so write it for them.

{access}

Supported: functions, closures, lambdas, simple classes (no inheritance), dataclasses,
comprehensions, try/except/finally, with, f-strings, and these modules:
json, math, datetime, re, collections, itertools, functools, dataclasses, typing, base64, copy,
random, unicodedata.
Not supported: generators/yield, match, del, inheritance, @property/@classmethod, user-defined
exception classes, eval/exec, %-formatting (\"x = %s\" % y — use an f-string), third-party packages,
and any module not listed above, including urllib, os, sys, time, hashlib, io, socket, subprocess.

Limits: {tools_timeout} wall-clock (the same tool timeout as every other tool), recursion depth 1000,
script size 64 KiB, any single allocation over 1 GiB (e.g. \"a\" * 10**10) raises MemoryError, and
print() output up to 10 MiB. Exceeding a limit ends the run with an error naming the limit.
Return plain data (str, int, float, bool, None, list, dict); other objects cannot be returned.
Keep results small: everything you return or print is delivered to you verbatim, so print what you
need to read rather than whole documents — slice, filter and summarise inside the script.
On error you get the exception and a traceback with line numbers — fix the script and re-run."
    )
}
