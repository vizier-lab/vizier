//! Drives one `MontyRun` to completion on a blocking-pool thread.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use monty::{MontyRun, RunProgress};
use monty_types::{
    CompileOptions, DEFAULT_MAX_PRINT_COLLECT_BYTES, ExcType, ExtFunctionResult, MontyException,
    MontyObject, NameLookupResult, PrintWriter, ResourceLimits, ResourceTracker,
};

use super::{
    BridgeError, MAX_SCRIPT_BYTES, SINGLE_ALLOCATION_GUARD, SandboxBridge, SandboxLimits,
    ToolFunctionDoc,
    convert::{json_to_monty, monty_to_json},
    docs,
    report::{ExecutionErrorKind, ExecutionReport},
};

/// Monty's CPU clock never runs ahead of wall time, so with this grace the agent
/// loop's own tool timeout always fires first and the turn gets the same
/// "timed out" error as any other tool; the clock only stops an orphaned spinning
/// thread, well within one more timeout (FR-021).
const CPU_CLOCK_GRACE: Duration = Duration::from_millis(500);

/// Sets the flag when the `execute` future is dropped (e.g. by the agent loop's
/// tool timeout), so the orphaned interpreter thread stops at its next host call.
struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Run `code` in a fresh interpreter. Never fails: every problem the script can
/// cause comes back as a report with `error` set, because the agent must read it.
pub async fn execute(
    code: &str,
    limits: SandboxLimits,
    bridge: Arc<dyn SandboxBridge>,
) -> ExecutionReport {
    if code.len() > MAX_SCRIPT_BYTES {
        return ExecutionReport::failed(
            ExecutionErrorKind::Limit,
            format!(
                "script is {} bytes; the limit is {MAX_SCRIPT_BYTES} bytes",
                code.len()
            ),
            "",
            Some("script_size"),
        );
    }

    let cancelled = Arc::new(AtomicBool::new(false));
    let _guard = CancelOnDrop(cancelled.clone());
    let handle = tokio::runtime::Handle::current();
    let span = tracing::Span::current();
    let code = code.to_string();

    let joined = tokio::task::spawn_blocking(move || {
        let _entered = span.enter();
        let started = Instant::now();
        let mut report = Interpreter {
            limits,
            bridge,
            handle,
            cancelled,
            last_tool_error: None,
        }
        .run(code);
        report.duration_ms = started.elapsed().as_millis() as u64;
        span.record("duration_ms", report.duration_ms);
        report
    })
    .await;

    joined.unwrap_or_else(|err| {
        tracing::error!("python sandbox panicked: {err}");
        ExecutionReport::failed(
            ExecutionErrorKind::Script,
            format!("sandbox panicked: {err}"),
            "",
            None,
        )
    })
}

/// Everything the resume loop needs; lives only on the interpreter thread.
struct Interpreter {
    limits: SandboxLimits,
    bridge: Arc<dyn SandboxBridge>,
    handle: tokio::runtime::Handle,
    cancelled: Arc<AtomicBool>,
    /// Message of the last tool failure raised into the script, so an uncaught
    /// one is reported as `Tool` rather than `Script`.
    last_tool_error: Option<String>,
}

fn runtime_error(message: impl Into<String>) -> MontyException {
    MontyException::new(ExcType::RuntimeError, Some(message.into()))
}

fn cancelled_error() -> MontyException {
    MontyException::new(
        ExcType::TimeoutError,
        Some("the tool timeout expired while the script was running".into()),
    )
}

impl Interpreter {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Every `MontyRun` and `MontyObject` is created and dropped inside this
    /// function; only the plain-data report leaves it.
    fn run(mut self, code: String) -> ExecutionReport {
        let started = Instant::now();
        let runner = match MontyRun::new(code, "main.py", vec![], CompileOptions::default()) {
            Ok(runner) => runner,
            Err(exc) => return ExecutionReport::from_monty_exception(&exc, started.elapsed()),
        };
        let tracker = ResourceTracker::new(
            ResourceLimits::default()
                .max_duration(self.limits.timeout + CPU_CLOCK_GRACE)
                .max_memory(SINGLE_ALLOCATION_GUARD)
                // Host-enforced in Monty; deliberately not counted here — the
                // timeout is the only bound on host round-trips.
                .max_suspensions(usize::MAX),
        );

        let mut stdout = String::new();
        macro_rules! pw {
            () => {
                PrintWriter::CollectString(&mut stdout, Some(DEFAULT_MAX_PRINT_COLLECT_BYTES))
            };
        }

        let mut progress = runner.start(vec![], tracker, pw!());
        let outcome = loop {
            let step = match progress {
                Ok(step) => step,
                Err(exc) => break Err(exc),
            };
            progress = match step {
                RunProgress::Complete(value) => break Ok(value),
                RunProgress::FunctionCall(call) => {
                    if self.is_cancelled() {
                        call.abort(cancelled_error(), pw!())
                    } else {
                        let result =
                            self.function_call(&call.function_name, &call.args, &call.kwargs);
                        call.resume(result, pw!())
                    }
                }
                RunProgress::NameLookup(lookup) => {
                    if self.is_cancelled() {
                        lookup.abort(cancelled_error(), pw!())
                    } else {
                        lookup.resume(NameLookupResult::Undefined, pw!())
                    }
                }
                RunProgress::OsCall(os_call) => {
                    let exc = if self.is_cancelled() {
                        cancelled_error()
                    } else {
                        runtime_error("filesystem/OS access is not available in the sandbox")
                    };
                    os_call.abort(exc, pw!())
                }
                RunProgress::ResolveFutures(futures) => futures.abort(
                    runtime_error("awaiting external futures is not available in the sandbox"),
                    pw!(),
                ),
            };
        };

        let mut report = match outcome {
            Ok(value) => match monty_to_json(&value) {
                Ok(result) => ExecutionReport {
                    ok: true,
                    result,
                    stdout: String::new(),
                    error: None,
                    tool_calls: vec![],
                    duration_ms: 0,
                },
                Err(err) => {
                    ExecutionReport::failed(ExecutionErrorKind::Script, err.to_string(), "", None)
                }
            },
            Err(exc) => {
                let mut report = ExecutionReport::from_monty_exception(&exc, started.elapsed());
                let from_tool = exc.exc_type() == ExcType::RuntimeError
                    && exc.message().is_some()
                    && exc.message() == self.last_tool_error.as_deref();
                if let (true, Some(error)) = (from_tool, report.error.as_mut()) {
                    error.kind = ExecutionErrorKind::Tool;
                }
                report
            }
        };
        report.stdout = stdout;
        report
    }

    fn function_call(
        &mut self,
        name: &str,
        args: &[MontyObject],
        kwargs: &[(MontyObject, MontyObject)],
    ) -> ExtFunctionResult {
        match name {
            "execute_python" => runtime_error("nested execute_python is not allowed").into(),
            // `list_tool_functions`/`describe_tool_function` are the model-side tool
            // names; models reach for them inside scripts too, so both spellings work.
            "list_tools" | "list_tool_functions" => self.list_tools(args, kwargs),
            "describe_tool" | "describe_tool_function" => self.describe_tool(args, kwargs),
            _ if !self.limits.tools_enabled => ExtFunctionResult::NotFound(name.to_string()),
            _ => self.tool_call(name, args, kwargs),
        }
    }

    fn catalogue(&self) -> Vec<ToolFunctionDoc> {
        if self.limits.tools_enabled {
            self.handle.block_on(self.bridge.catalogue())
        } else {
            vec![]
        }
    }

    fn list_tools(
        &self,
        args: &[MontyObject],
        kwargs: &[(MontyObject, MontyObject)],
    ) -> ExtFunctionResult {
        if !args.is_empty() || !kwargs.is_empty() {
            return type_error("list_tools() takes no arguments").into();
        }
        let entries = self
            .catalogue()
            .into_iter()
            .map(|doc| serde_json::json!({ "function": doc.function, "summary": doc.summary }))
            .collect();
        json_to_monty(serde_json::Value::Array(entries)).into()
    }

    fn describe_tool(
        &self,
        args: &[MontyObject],
        kwargs: &[(MontyObject, MontyObject)],
    ) -> ExtFunctionResult {
        let function = match (args, kwargs) {
            ([MontyObject::String(function)], []) => function,
            ([], [(MontyObject::String(key), MontyObject::String(function))]) if key == "name" => {
                function
            }
            _ => {
                return type_error("describe_tool() takes one argument: the function name").into();
            }
        };
        let doc = if self.limits.tools_enabled {
            self.handle.block_on(self.bridge.describe(function))
        } else {
            None
        };
        let value = match doc {
            Some(doc) => serde_json::to_value(doc).unwrap_or_default(),
            None => {
                let known: Vec<String> =
                    self.catalogue().into_iter().map(|doc| doc.function).collect();
                docs::not_found(function, &known)
            }
        };
        json_to_monty(value).into()
    }

    fn tool_call(
        &mut self,
        name: &str,
        args: &[MontyObject],
        kwargs: &[(MontyObject, MontyObject)],
    ) -> ExtFunctionResult {
        let arguments = match self.arguments(name, args, kwargs) {
            Ok(Some(arguments)) => arguments,
            Ok(None) => return ExtFunctionResult::NotFound(name.to_string()),
            Err(exc) => return exc.into(),
        };
        match self.handle.block_on(self.bridge.call(name, arguments)) {
            Ok(value) => json_to_monty(value).into(),
            Err(BridgeError::UnknownFunction) => ExtFunctionResult::NotFound(name.to_string()),
            Err(BridgeError::Tool(message)) => {
                let message = format!("{name}: {message}");
                self.last_tool_error = Some(message.clone());
                runtime_error(message).into()
            }
        }
    }

    /// The JSON object handed to the tool: keyword arguments verbatim, positional
    /// ones mapped onto the input schema's `required` order. `Ok(None)` when the
    /// function does not exist.
    fn arguments(
        &self,
        name: &str,
        args: &[MontyObject],
        kwargs: &[(MontyObject, MontyObject)],
    ) -> Result<Option<serde_json::Value>, MontyException> {
        let convert = |param: &str, value: &MontyObject| {
            monty_to_json(value).map_err(|err| {
                type_error(format!(
                    "{name}() argument '{param}' is a {}; pass plain data (str, int, float, bool, None, list, dict)",
                    err.python_type
                ))
            })
        };

        let mut object = serde_json::Map::new();
        if !args.is_empty() {
            let Some(doc) = self.handle.block_on(self.bridge.describe(name)) else {
                return Ok(None);
            };
            let required: Vec<&str> = doc
                .parameters
                .iter()
                .filter(|p| p.required)
                .map(|p| p.name.as_str())
                .collect();
            if required.is_empty() || args.len() > required.len() {
                return Err(type_error(format!(
                    "{name}() takes keyword arguments only; see describe_tool('{name}')"
                )));
            }
            for (param, value) in required.iter().zip(args) {
                object.insert(param.to_string(), convert(param, value)?);
            }
        }
        for (key, value) in kwargs {
            let MontyObject::String(key) = key else {
                return Err(type_error(format!("{name}() keywords must be strings")));
            };
            if object.contains_key(key) {
                return Err(type_error(format!(
                    "{name}() got multiple values for argument '{key}'"
                )));
            }
            object.insert(key.clone(), convert(key, value)?);
        }
        Ok(Some(serde_json::Value::Object(object)))
    }
}

fn type_error(message: impl Into<String>) -> MontyException {
    MontyException::new(ExcType::TypeError, Some(message.into()))
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::sandbox::NoToolsBridge;

    fn limits(timeout_ms: u64) -> SandboxLimits {
        SandboxLimits {
            timeout: Duration::from_millis(timeout_ms),
            tools_enabled: false,
        }
    }

    async fn run(code: &str) -> ExecutionReport {
        execute(code, limits(5_000), Arc::new(NoToolsBridge)).await
    }

    fn error_of(report: &ExecutionReport) -> &crate::sandbox::ExecutionError {
        assert!(!report.ok, "expected failure, got {report:?}");
        report.error.as_ref().expect("error set when !ok")
    }

    #[tokio::test]
    async fn last_expression_is_the_result() {
        let report = run("1 + 1").await;
        assert!(report.ok, "{report:?}");
        assert_eq!(report.result, serde_json::json!(2));
    }

    #[tokio::test]
    async fn print_is_captured() {
        let report = run("print('hi')\nx = 1").await;
        assert!(report.ok, "{report:?}");
        assert_eq!(report.stdout, "hi\n");
        assert_eq!(report.result, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn empty_script_succeeds_with_null() {
        let report = run("").await;
        assert!(report.ok, "{report:?}");
        assert_eq!(report.result, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn syntax_error_is_a_script_error() {
        let report = run("def f(:").await;
        let err = error_of(&report);
        assert_eq!(err.kind, ExecutionErrorKind::Script);
        assert!(err.message.contains("SyntaxError"), "{err:?}");
    }

    #[tokio::test]
    async fn infinite_loop_hits_the_timeout() {
        let report = execute("while True: pass", limits(50), Arc::new(NoToolsBridge)).await;
        let err = error_of(&report);
        assert_eq!(err.kind, ExecutionErrorKind::Limit);
        assert_eq!(err.limit.as_deref(), Some("timeout"));
    }

    #[tokio::test]
    async fn unbounded_recursion_hits_the_recursion_limit() {
        let report = run("def f(n): return f(n+1)\nf(0)").await;
        let err = error_of(&report);
        assert_eq!(err.kind, ExecutionErrorKind::Limit);
        assert_eq!(err.limit.as_deref(), Some("recursion"));
    }

    #[tokio::test]
    async fn filesystem_access_is_refused() {
        let report = run("open('/etc/passwd').read()").await;
        let err = error_of(&report);
        assert_eq!(err.kind, ExecutionErrorKind::Script);
        assert!(err.message.contains("not available"), "{err:?}");
    }

    #[tokio::test]
    async fn environment_access_is_refused() {
        let report = run("import os\nos.getenv('HOME')").await;
        let err = error_of(&report);
        assert!(err.message.contains("not available"), "{err:?}");
    }

    #[tokio::test]
    async fn network_modules_do_not_exist() {
        let report = run("import socket").await;
        let err = error_of(&report);
        assert_eq!(err.kind, ExecutionErrorKind::Script);
        assert!(err.message.contains("ModuleNotFoundError"), "{err:?}");
    }

    #[tokio::test]
    async fn tools_are_undefined_without_code_mode() {
        let report = run("memory_read(query='x')").await;
        let err = error_of(&report);
        assert_eq!(err.kind, ExecutionErrorKind::Script);
        assert!(err.message.contains("NameError"), "{err:?}");
    }

    #[tokio::test]
    async fn nested_execute_python_is_refused() {
        let report = run("execute_python(code='1')").await;
        let err = error_of(&report);
        assert_eq!(err.kind, ExecutionErrorKind::Script);
        assert!(err.message.contains("nested"), "{err:?}");
    }

    #[tokio::test]
    async fn oversized_allocation_hits_the_memory_guard() {
        let report = run("'a' * (2**31)").await;
        let err = error_of(&report);
        assert_eq!(err.kind, ExecutionErrorKind::Limit);
        assert_eq!(err.limit.as_deref(), Some("memory"));
    }

    #[tokio::test]
    async fn class_instances_cannot_be_returned() {
        let report = run("class A:\n    pass\nA()").await;
        let err = error_of(&report);
        assert_eq!(err.kind, ExecutionErrorKind::Script);
        assert!(err.message.contains("cannot return"), "{err:?}");
    }

    #[tokio::test]
    async fn oversized_scripts_are_refused_before_compiling() {
        let code = "#".repeat(MAX_SCRIPT_BYTES + 1);
        let report = run(&code).await;
        let err = error_of(&report);
        assert_eq!(err.kind, ExecutionErrorKind::Limit);
        assert_eq!(err.limit.as_deref(), Some("script_size"));
    }

    #[tokio::test]
    async fn runs_are_stateless() {
        assert!(run("y = 1").await.ok);
        let err = run("y").await;
        assert!(error_of(&err).message.contains("NameError"));
    }

    /// Records calls, returns canned JSON, fails `broken`, and documents
    /// `search(query, limit)` with `query` required.
    struct FakeBridge {
        calls: std::sync::Mutex<Vec<(String, serde_json::Value)>>,
    }

    impl FakeBridge {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                calls: std::sync::Mutex::new(vec![]),
            })
        }

        fn calls(&self) -> Vec<(String, serde_json::Value)> {
            self.calls.lock().unwrap().clone()
        }
    }

    fn doc(function: &str, required: &[&str]) -> ToolFunctionDoc {
        ToolFunctionDoc {
            function: function.into(),
            tool: function.into(),
            summary: format!("{function} summary."),
            description: String::new(),
            parameters: required
                .iter()
                .map(|name| crate::sandbox::docs::ParamDoc {
                    name: name.to_string(),
                    r#type: "string".into(),
                    required: true,
                    description: String::new(),
                })
                .collect(),
            returns: "any JSON value".into(),
            example: String::new(),
        }
    }

    #[async_trait::async_trait]
    impl SandboxBridge for FakeBridge {
        async fn catalogue(&self) -> Vec<ToolFunctionDoc> {
            vec![doc("broken", &[]), doc("echo", &[]), doc("search", &["query"])]
        }

        async fn describe(&self, function: &str) -> Option<ToolFunctionDoc> {
            self.catalogue().await.into_iter().find(|d| d.function == function)
        }

        async fn call(
            &self,
            function: &str,
            arguments: serde_json::Value,
        ) -> Result<serde_json::Value, BridgeError> {
            self.calls
                .lock()
                .unwrap()
                .push((function.to_string(), arguments.clone()));
            match function {
                "broken" => Err(BridgeError::Tool("it broke".into())),
                "echo" | "search" => Ok(serde_json::json!({ "echo": arguments })),
                _ => Err(BridgeError::UnknownFunction),
            }
        }
    }

    async fn run_with(code: &str, bridge: Arc<FakeBridge>, tools_enabled: bool) -> ExecutionReport {
        let limits = SandboxLimits {
            timeout: Duration::from_secs(5),
            tools_enabled,
        };
        execute(code, limits, bridge).await
    }

    #[tokio::test]
    async fn kwargs_arrive_verbatim() {
        let bridge = FakeBridge::new();
        let report = run_with("echo(a=1, b=[True, None], c={'k': 'v'})", bridge.clone(), true).await;
        assert!(report.ok, "{report:?}");
        assert_eq!(
            bridge.calls(),
            vec![("echo".into(), serde_json::json!({"a": 1, "b": [true, null], "c": {"k": "v"}}))]
        );
        assert_eq!(report.result["echo"]["a"], 1);
    }

    #[tokio::test]
    async fn positionals_map_onto_required_parameters() {
        let bridge = FakeBridge::new();
        let report = run_with("search('rust', limit=3)", bridge.clone(), true).await;
        assert!(report.ok, "{report:?}");
        assert_eq!(bridge.calls()[0].1, serde_json::json!({"query": "rust", "limit": 3}));
    }

    #[tokio::test]
    async fn positionals_without_required_parameters_are_a_type_error() {
        let report = run_with("echo('x')", FakeBridge::new(), true).await;
        let err = error_of(&report);
        assert_eq!(err.kind, ExecutionErrorKind::Script);
        assert!(err.message.contains("TypeError"), "{err:?}");
        assert!(err.message.contains("keyword arguments only"), "{err:?}");
    }

    #[tokio::test]
    async fn tool_errors_are_catchable_runtime_errors() {
        let code = "try:\n    broken()\nexcept RuntimeError as e:\n    r = str(e)\nr";
        let report = run_with(code, FakeBridge::new(), true).await;
        assert!(report.ok, "{report:?}");
        assert_eq!(report.result, serde_json::json!("broken: it broke"));
    }

    #[tokio::test]
    async fn uncaught_tool_errors_are_tool_kind() {
        let report = run_with("broken()", FakeBridge::new(), true).await;
        let err = error_of(&report);
        assert_eq!(err.kind, ExecutionErrorKind::Tool);
        assert!(err.message.contains("broken: it broke"), "{err:?}");
    }

    #[tokio::test]
    async fn calls_happen_in_order() {
        let bridge = FakeBridge::new();
        let report = run_with("[echo(i=i) for i in range(3)]", bridge.clone(), true).await;
        assert!(report.ok, "{report:?}");
        let seen: Vec<_> = bridge.calls().into_iter().map(|(_, args)| args["i"].clone()).collect();
        assert_eq!(seen, vec![serde_json::json!(0), serde_json::json!(1), serde_json::json!(2)]);
    }

    #[tokio::test]
    async fn host_round_trips_are_not_capped() {
        let bridge = FakeBridge::new();
        let report = run_with("for i in range(2000):\n    echo(i=i)\n'done'", bridge.clone(), true).await;
        assert!(report.ok, "{report:?}");
        assert_eq!(bridge.calls().len(), 2000);
    }

    #[tokio::test]
    async fn a_dropped_run_aborts_at_the_next_host_call() {
        let bridge = FakeBridge::new();
        let limits = SandboxLimits {
            timeout: Duration::from_secs(30),
            tools_enabled: true,
        };
        let run = execute("while True:\n    echo()", limits, bridge.clone());
        // The agent loop's tool timeout drops the future like this.
        assert!(tokio::time::timeout(Duration::from_millis(100), run).await.is_err());
        let seen = bridge.calls().len();
        tokio::time::sleep(Duration::from_millis(200)).await;
        let after = bridge.calls().len();
        assert!(after <= seen + 1, "orphan kept calling tools: {seen} -> {after}");
    }

    #[tokio::test]
    async fn tools_are_undefined_when_code_mode_is_off() {
        let bridge = FakeBridge::new();
        let report = run_with("echo(a=1)", bridge.clone(), false).await;
        assert!(error_of(&report).message.contains("NameError"));
        assert!(bridge.calls().is_empty());
    }

    #[tokio::test]
    async fn unknown_functions_are_name_errors() {
        let report = run_with("nope(a=1)", FakeBridge::new(), true).await;
        assert!(error_of(&report).message.contains("NameError"));
    }

    #[tokio::test]
    async fn list_tools_returns_the_catalogue() {
        let report = run_with("[d['function'] for d in list_tools()]", FakeBridge::new(), true).await;
        assert!(report.ok, "{report:?}");
        assert_eq!(report.result, serde_json::json!(["broken", "echo", "search"]));
        let report = run_with("list_tools()", FakeBridge::new(), false).await;
        assert_eq!(report.result, serde_json::json!([]));
    }

    #[tokio::test]
    async fn describe_tool_finds_or_suggests() {
        let report = run_with("describe_tool('search')['parameters'][0]['name']", FakeBridge::new(), true).await;
        assert_eq!(report.result, serde_json::json!("query"));
        let report = run_with("describe_tool(name='serch')", FakeBridge::new(), true).await;
        assert!(report.ok, "{report:?}");
        assert_eq!(report.result["available"], serde_json::json!(false));
        assert_eq!(report.result["did_you_mean"][0], serde_json::json!("search"));
        let report = run_with("describe_tool()", FakeBridge::new(), true).await;
        assert!(error_of(&report).message.contains("TypeError"));
    }

    #[tokio::test]
    async fn the_model_side_tool_names_work_inside_a_script_too() {
        let report = run_with(
            "[d['function'] for d in list_tool_functions()]",
            FakeBridge::new(),
            true,
        )
        .await;
        assert!(report.ok, "{report:?}");
        assert_eq!(report.result, serde_json::json!(["broken", "echo", "search"]));

        let report = run_with(
            "describe_tool_function(name='search')['function']",
            FakeBridge::new(),
            true,
        )
        .await;
        assert_eq!(report.result, serde_json::json!("search"));
    }
}
