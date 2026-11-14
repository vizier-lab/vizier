//! The Python sandbox tools: `execute_python` and the two documentation tools. The engine itself lives in `crate::sandbox`.

use std::sync::Arc;


use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::Instrument;

use crate::{
    agents::tools::{ToolCategory, ToolContext, ToolRouter, VizierTool},
    error::VizierError,
    sandbox::{self, ExecutionReport, NoToolsBridge, SandboxBridge, SandboxLimits},
    schema::{VizierAttachment, VizierResponse, VizierResponseContent},
};

mod bridge;
mod docs_tools;

use bridge::RouterBridge;
pub use docs_tools::{DescribeToolFunction, ListToolFunctions, ListToolFunctionsInput};

pub struct ExecutePython {
    router: ToolRouter,
    limits: SandboxLimits,
    description: String,
    detached: Option<crate::agents::detached::DetachedScripts>,
}

impl ExecutePython {
    pub fn new(router: ToolRouter, limits: SandboxLimits, tools_timeout: &str) -> Self {
        Self {
            router,
            limits,
            description: execute_python_description(tools_timeout),
            detached: None,
        }
    }

    /// Where a `detached: true` call goes; without it such a call is refused.
    pub fn with_detached(mut self, detached: Option<crate::agents::detached::DetachedScripts>) -> Self {
        self.detached = detached;
        self
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
    #[schemars(
        description = "One short sentence, in plain language, saying what this \
         script is for. It is shown to the person watching in place of the code — write it \
         for them, not for yourself. E.g. \"compute the 99th percentile from the last 24h \
         of latency samples\"."
    )]
    pub intent: String,

    #[schemars(
        description = "Run the script in the background and get a handle back at once          (`{\"detached\": true, \"id\": \"d_…\"}`). Keep working; the script's result is delivered          to you later as a `[Background work finished]` message. Default false."
    )]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub detached: bool,

    #[schemars(
        description = "With `detached`: the whole script's time limit in seconds, tool and          sub-agent waits included, 1 to 3600. Default 600. Ignored without `detached`."
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

impl ExecutePython {
    /// Record a detached script, start it, and hand back its handle (spec 058
    /// contracts/detached-flag.md §1). The script runs with the same router and tools, under
    /// its own limit for the whole script and for each call inside it.
    async fn launch_detached(
        &self,
        args: ExecutePythonInput,
        timeout_secs: u64,
        ctx: &ToolContext,
    ) -> Result<VizierResponse, VizierError> {
        let Some(detached) = &self.detached else {
            return Err(VizierError("cannot detach here: background work is not available".into()));
        };
        let router = self.router.clone();
        let limits = SandboxLimits {
            timeout: std::time::Duration::from_secs(timeout_secs),
            tools_enabled: self.limits.tools_enabled,
        };
        let code = args.code.clone();
        let run: crate::agents::detached::ScriptRunner = Box::new(move |ctx, progress| {
            Box::pin(async move {
                run_script(&router, &code, limits, limits.timeout, &ctx, Some(progress)).await
            })
        });
        let script = detached
            .launch(
                ctx,
                crate::agents::detached::Launch {
                    intent: args.intent,
                    code: args.code,
                    timeout_secs,
                },
                run,
            )
            .await?;
        Ok(VizierResponse {
            timestamp: crate::time::now(),
            content: VizierResponseContent::ToolResponse {
                response: serde_json::json!({
                    "detached": true,
                    "id": script.id,
                    "state": script.state,
                    "timeout_secs": script.timeout_secs,
                }),
            },
            attachments: vec![],
            ..Default::default()
        })
    }
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

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        // `args.intent` deliberately stops here: only the code reaches the sandbox, so the
        // stated intent cannot become a script variable or affect limits, tool availability
        // or the result. It is read only by whatever renders the call, from the tool call's
        // arguments — which is also why it is not copied onto `ExecutionReport`.
        let timeout_secs = crate::agents::detached::validate_timeout(args.detached, args.timeout_secs)?;
        if args.detached {
            return self.launch_detached(args, timeout_secs, ctx).await;
        }
        let (report, attachments) =
            run_script(&self.router, &args.code, self.limits, self.limits.timeout, ctx, None).await;

        Ok(VizierResponse {
            timestamp: crate::time::now(),
            content: VizierResponseContent::ToolResponse {
                response: serde_json::to_value(report)
                    .map_err(|err| VizierError(err.to_string()))?,
            },
            attachments,
            ..Default::default()
        })
    }
}

pub use crate::agents::detached::ProgressFn;

/// One script run, inline or detached: build the bridge, run under `limits`, collect the
/// nested calls and attachments, and scrub the report. Every nested call is cut off after
/// `tool_timeout`. Never fails; a failing script is a report with `error` set.
pub(crate) async fn run_script(
    router: &ToolRouter,
    code: &str,
    limits: SandboxLimits,
    tool_timeout: std::time::Duration,
    ctx: &ToolContext,
    progress: Option<ProgressFn>,
) -> (ExecutionReport, Vec<VizierAttachment>) {
    let span = tracing::info_span!(
        "python_exec",
        agent_id = %ctx.session.0,
        session = %ctx.session.to_slug(),
        duration_ms = tracing::field::Empty,
    );

    let bridge = limits.tools_enabled.then(|| {
        let bridge = RouterBridge::new(router.clone(), ctx.clone(), tool_timeout);
        Arc::new(match progress {
            Some(progress) => bridge.with_progress(progress),
            None => bridge,
        })
    });
    let engine_bridge: Arc<dyn SandboxBridge> = match &bridge {
        Some(bridge) => bridge.clone(),
        None => Arc::new(NoToolsBridge),
    };

    let mut report = sandbox::execute(code, limits, engine_bridge)
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

    // The script got the real values; everything that leaves this point — the log line,
    // the returned report, its history entry and the model's copy — is scrubbed first
    // (`specs/027-agent-secrets`). A turn that handled no secret pays one empty check.
    ctx.secret_redactor.report(&mut report);

    span.in_scope(|| match &report.error {
        None => tracing::info!(
            ok = true,
            duration_ms = report.duration_ms,
            "python script finished"
        ),
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
    (report, attachments)
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
On error you get the exception and a traceback with line numbers — fix the script and re-run.

Background: with `detached: true` the script runs in the background and this call returns a handle
({{\"detached\": true, \"id\": \"d_…\"}}) at once; `timeout_secs` (1 to 3600, default 600) then bounds the
whole script, every call inside it included. Its result arrives later as a `[Background work finished]`
message. A detached script cannot ask the person, use client tools or end the turn. Put work you want
back together in one script; run several detached scripts for work that should arrive separately."
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        agents::tools::VizierToolSet,
        questions::{AskReply, Questions, QuestionsHandle, tool_sensitive::AskSensitiveInput},
        schema::{
            SessionHistory, SessionHistoryContent, VizierChannelId, VizierSession,
            history_entries_to_messages,
        },
        storage::{VizierStorage, document::BlobDocumentStore, sqlite::SqlStorage},
        transport::VizierTransport,
    };

    const TOKEN: &str = "ghp_end-to-end-secret-4242";

    fn session() -> VizierSession {
        VizierSession(
            "agent-1".into(),
            VizierChannelId::HTTP("alice".into(), "webui".into()),
            Some("t".into()),
        )
    }

    /// The whole chain of US5: a script asks for a value, the person supplies it, the script uses
    /// it byte for byte, and no copy that leaves the turn carries it — not the report the model
    /// reads, not what the next turn is built from (SC-014, SC-015, SC-022, FR-041).
    #[tokio::test]
    async fn a_supplied_value_reaches_the_script_and_no_copy_that_leaves_the_turn() {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_question_schema(&conn).unwrap();
        crate::storage::sqlite::init_sensitive_ask_schema(&conn).unwrap();
        let storage = Arc::new(VizierStorage::new(SqlStorage::new(
            crate::storage::sql::SqlStore::sqlite(conn),
            Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
        )));
        let questions = Arc::new(Questions::new());

        let router = ToolRouter {
            default_toolset: VizierToolSet::new().tool(AskSensitiveInput::new(
                storage.clone(),
                VizierTransport::new(),
            )),
            user_toolset: VizierToolSet::new(),
            integration_toolset: VizierToolSet::new(),
            addons: Default::default(),
            mcp: Default::default(),
            reserved: Arc::new(vec![]),
        };
        let ctx = ToolContext {
            session: session(),
            pending_attachments: Default::default(),
            hooks: None,
            background_depth: 0,
            loaded_images: Default::default(),
            recalled: Default::default(),
            deadline: None,
            client: None,
            questions: QuestionsHandle::new(questions.clone()),
            interactive: true,
            secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
            dream_sources: vec![],
            project: None,
            detached: false,
            parent_detached_id: None,
        };
        let tool = ExecutePython::new(
            router,
            SandboxLimits {
                timeout: Duration::from_secs(20),
                tools_enabled: true,
            },
            "20s",
        );

        // The person: wait for the dialog, then type the value into it.
        let person = {
            let questions = questions.clone();
            tokio::spawn(async move {
                for _ in 0..400 {
                    if let Some(id) = questions.open_asks(&session()).into_iter().next() {
                        assert!(questions.answer(&id, AskReply::Value(TOKEN.into())));
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                panic!("the dialog never opened");
            })
        };

        // The script prints the value, returns it and compares it, which proves it received it
        // exactly while the report must not show it.
        let code = format!(
            "r = ask_sensitive_input(purpose='to push the branch')\n\
             print('got', r['value'])\n\
             {{'value': r['value'], 'exact': r['value'] == '{TOKEN}', 'protected': r['protected']}}"
        );
        let response = tool
            .call(
                ExecutePythonInput {
                    code,
                    intent: "ask for a token and use it".into(),
                    detached: false,
                    timeout_secs: None,
                },
                &ctx,
            )
            .await
            .expect("the tool call itself succeeds");
        person.await.unwrap();

        let VizierResponseContent::ToolResponse { response: report } = &response.content else {
            panic!("not a tool response: {:?}", response.content);
        };
        let report_text = report.to_string();
        assert_eq!(report["ok"], true, "{report_text}");
        // The script received the value exactly...
        assert_eq!(report["result"]["exact"], true, "{report_text}");
        assert_eq!(report["result"]["protected"], true, "{report_text}");
        // ...and nothing the model reads carries it, where it was used the marker stands.
        assert!(
            !report_text.contains(TOKEN),
            "the report carries the value: {report_text}"
        );
        assert!(report_text.contains("[redacted]"), "{report_text}");

        // What the next turn is built from: this turn's history entries, replayed.
        let entry = |content| SessionHistory {
            uid: uuid::Uuid::new_v4().to_string(),
            vizier_session: session(),
            content,
            timestamp: crate::time::now(),
            reactions: vec![],
            seq: None,
        };
        let entries = vec![
            entry(SessionHistoryContent::ToolCall {
                call_id: "c1".into(),
                name: "execute_python".into(),
                arguments: serde_json::json!({ "code": "…", "intent": "ask for a token" }),
            }),
            // The ask's own record, which a person sees and the model must not be told more from.
            entry(SessionHistoryContent::SensitiveAsk {
                ask_id: "a1".into(),
                purpose: "to push the branch".into(),
                label: None,
                state: crate::questions::model::AskState::Provided,
            }),
            entry(SessionHistoryContent::ToolResult {
                call_id: "c1".into(),
                content: report_text,
            }),
        ];
        let next_turn = serde_json::to_string(&history_entries_to_messages(&entries)).unwrap();
        assert!(
            !next_turn.contains(TOKEN),
            "the next turn is built with the value: {next_turn}"
        );
        assert!(next_turn.contains("[redacted]"), "{next_turn}");

        // Neither the ask's stored record nor the open slot keeps it.
        use crate::storage::question::SensitiveAskStorage;
        let stored = storage
            .sensitive_asks(&session(), crate::questions::model::AskState::Provided)
            .await
            .unwrap();
        assert_eq!(stored.len(), 1, "the ask is recorded as provided");
        assert!(!format!("{stored:?}").contains(TOKEN));
        assert!(questions.open_asks(&session()).is_empty());
    }
}
