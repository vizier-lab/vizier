//! The structured result of one `execute_python` run. It is the tool's output,
//! the persisted `ToolResult.content`, and the live WebUI event payload.

use std::time::Duration;

use monty_types::{ExcType, MontyException};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub ok: bool,
    /// JSON value of the script's last expression; `null` if none or on error.
    pub result: serde_json::Value,
    /// Captured `print()` output.
    pub stdout: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ExecutionError>,
    /// Ordered; empty when the run had no tool access.
    pub tool_calls: Vec<ToolInvocationRecord>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionError {
    pub kind: ExecutionErrorKind,
    /// e.g. "TypeError: unsupported operand type(s) for +: 'int' and 'str'"
    pub message: String,
    /// CPython-style traceback with line numbers; empty for host-raised limits.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub traceback: String,
    /// For `Limit`: "timeout" | "memory" | "recursion" | "script_size".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionErrorKind {
    Script,
    Tool,
    Limit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocationRecord {
    /// 1-based order within the script.
    pub seq: u32,
    /// The tool name as dispatched.
    pub name: String,
    pub arguments: serde_json::Value,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration_ms: u64,
    /// The dispatched function ends the turn when it succeeds (spec 026).
    #[serde(default, skip_serializing_if = "is_false")]
    pub ends_turn: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl ExecutionReport {
    /// Keys that identify a serialised report; the WebUI duck-types on the same set.
    pub const SIGNATURE_KEYS: [&'static str; 4] = ["ok", "stdout", "tool_calls", "duration_ms"];

    /// Whether a JSON value is a serialised `ExecutionReport`.
    pub fn looks_like(value: &serde_json::Value) -> bool {
        value
            .as_object()
            .is_some_and(|map| Self::SIGNATURE_KEYS.iter().all(|key| map.contains_key(*key)))
    }

    /// The report as the *model* should see it, given the serialised form that
    /// storage and the WebUI keep.
    ///
    /// Only each nested call's `arguments` is dropped: the script that made the call
    /// states them already, a few hundred tokens up in the same request, and they
    /// measured at 13.5% of report bytes. `stdout` is left whole — it is where a
    /// script actually answers — so the one remaining bound on it is the engine's
    /// 10 MiB print budget.
    ///
    /// Returns `None` for a value that is not a report, so callers can pass any
    /// tool result through.
    pub fn model_view(value: &Value) -> Option<Value> {
        if !Self::looks_like(value) {
            return None;
        }
        let mut value = value.clone();
        let map = value.as_object_mut()?;

        if let Some(calls) = map.get_mut("tool_calls").and_then(Value::as_array_mut) {
            for call in calls {
                if let Some(call) = call.as_object_mut() {
                    call.remove("arguments");
                }
            }
        }

        Some(value)
    }

    /// Whether this run is a turn-ending one: the script finished and a function that
    /// ends the turn succeeded in it.
    pub fn ended_turn(&self) -> bool {
        self.ok && self.tool_calls.iter().any(|call| call.ok && call.ends_turn)
    }

    /// The name of the function that ended the turn in a serialised report, if one did.
    /// Reads the JSON rather than deserialising it, since the model's copy of a report has
    /// lost each call's `arguments`. `None` for anything that is not a turn-ending report.
    pub fn ending_call(value: &Value) -> Option<String> {
        if !Self::looks_like(value) || value.get("ok") != Some(&Value::Bool(true)) {
            return None;
        }
        value
            .get("tool_calls")?
            .as_array()?
            .iter()
            .rev()
            .find(|call| {
                call.get("ok") == Some(&Value::Bool(true))
                    && call.get("ends_turn") == Some(&Value::Bool(true))
            })
            .and_then(|call| call.get("name")?.as_str().map(str::to_string))
    }

    pub fn failed(
        kind: ExecutionErrorKind,
        message: impl Into<String>,
        traceback: impl Into<String>,
        limit: Option<&str>,
    ) -> Self {
        Self {
            ok: false,
            result: serde_json::Value::Null,
            stdout: String::new(),
            error: Some(ExecutionError {
                kind,
                message: message.into(),
                traceback: traceback.into(),
                limit: limit.map(str::to_string),
            }),
            tool_calls: vec![],
            duration_ms: 0,
        }
    }

    pub fn from_monty_exception(exc: &MontyException, duration: Duration) -> Self {
        let limit = match exc.exc_type() {
            ExcType::TimeoutError => Some("timeout"),
            ExcType::MemoryError => Some("memory"),
            ExcType::RecursionError => Some("recursion"),
            _ => None,
        };
        let kind = if limit.is_some() {
            ExecutionErrorKind::Limit
        } else {
            ExecutionErrorKind::Script
        };
        let mut report = Self::failed(kind, exc.summary(), exc.to_string(), limit);
        report.duration_ms = duration.as_millis() as u64;
        report
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn report() -> ExecutionReport {
        ExecutionReport {
            ok: true,
            result: json!({ "hits": 2 }),
            stdout: "line one\nline two\n".into(),
            error: None,
            tool_calls: vec![ToolInvocationRecord {
                seq: 1,
                name: "memory_read".into(),
                arguments: json!({ "query": "rust releases", "limit": 5 }),
                ok: true,
                error: None,
                duration_ms: 12,
                ends_turn: false,
            }],
            duration_ms: 34,
        }
    }

    #[test]
    fn a_clean_report_serialises_without_empty_fields() {
        let value = serde_json::to_value(report()).unwrap();
        let map = value.as_object().unwrap();

        assert!(!map.contains_key("error"));
        for key in ExecutionReport::SIGNATURE_KEYS {
            assert!(map.contains_key(key), "{key} must stay: it identifies a report");
        }
        let call = value["tool_calls"][0].as_object().unwrap();
        assert!(!call.contains_key("error"));
    }

    #[test]
    fn a_host_raised_limit_serialises_without_a_traceback() {
        let value =
            serde_json::to_value(ExecutionReport::failed(
                ExecutionErrorKind::Limit,
                "timed out",
                "",
                Some("timeout"),
            ))
            .unwrap();

        let error = value["error"].as_object().unwrap();
        assert_eq!(error["limit"], "timeout");
        assert!(!error.contains_key("traceback"));
    }

    #[test]
    fn the_model_view_drops_nested_arguments_and_keeps_everything_else() {
        let full = serde_json::to_value(report()).unwrap();
        let view = ExecutionReport::model_view(&full).unwrap();

        let call = view["tool_calls"][0].as_object().unwrap();
        assert!(!call.contains_key("arguments"));
        assert_eq!(call["name"], "memory_read");
        assert_eq!(call["seq"], 1);
        assert_eq!(call["ok"], true);
        assert_eq!(call["duration_ms"], 12);

        assert_eq!(view["result"], full["result"]);
        assert_eq!(view["stdout"], full["stdout"]);
        assert_eq!(view["ok"], full["ok"]);
        assert_eq!(view["duration_ms"], full["duration_ms"]);

        // The record handed to storage and the WebUI is untouched.
        assert_eq!(full["tool_calls"][0]["arguments"]["query"], "rust releases");
    }

    #[test]
    fn a_long_stdout_reaches_the_model_whole() {
        let mut report = report();
        report.stdout = "x".repeat(200_000);
        let full = serde_json::to_value(report).unwrap();

        let view = ExecutionReport::model_view(&full).unwrap();
        assert_eq!(view["stdout"].as_str().unwrap().len(), 200_000);
    }

    #[test]
    fn the_model_view_passes_over_anything_that_is_not_a_report() {
        assert!(ExecutionReport::model_view(&json!({ "slug": "a-memory" })).is_none());
        assert!(ExecutionReport::model_view(&json!("plain text")).is_none());
    }

    #[test]
    fn a_round_trip_survives_the_skipped_fields() {
        let value = serde_json::to_value(report()).unwrap();
        let back: ExecutionReport = serde_json::from_value(value).unwrap();

        assert!(back.error.is_none());
        assert!(back.tool_calls[0].error.is_none());
        assert_eq!(back.tool_calls[0].arguments["limit"], 5);
    }

    fn call(ok: bool, ends_turn: bool) -> ToolInvocationRecord {
        ToolInvocationRecord {
            seq: 1,
            name: "ui_render".into(),
            arguments: json!({}),
            ok,
            error: None,
            duration_ms: 1,
            ends_turn,
        }
    }

    #[test]
    fn a_turn_ends_only_when_the_script_and_a_flagged_call_both_succeeded() {
        let mut ended = report();
        ended.tool_calls = vec![call(false, false), call(true, true)];
        assert!(ended.ended_turn());

        let mut failed_call = report();
        failed_call.tool_calls = vec![call(false, true)];
        assert!(!failed_call.ended_turn());

        let mut unflagged = report();
        unflagged.tool_calls = vec![call(true, false)];
        assert!(!unflagged.ended_turn());

        let mut failed_script = report();
        failed_script.ok = false;
        failed_script.tool_calls = vec![call(true, true)];
        assert!(!failed_script.ended_turn());

        let mut no_calls = report();
        no_calls.tool_calls = vec![];
        assert!(!no_calls.ended_turn());
    }

    #[test]
    fn the_ending_call_is_read_from_a_serialised_report_without_arguments() {
        let mut ended = report();
        ended.tool_calls = vec![call(true, false), call(true, true)];
        let full = serde_json::to_value(&ended).unwrap();
        let view = ExecutionReport::model_view(&full).unwrap();

        assert_eq!(ExecutionReport::ending_call(&full).as_deref(), Some("ui_render"));
        assert_eq!(ExecutionReport::ending_call(&view).as_deref(), Some("ui_render"));
        assert_eq!(ExecutionReport::ending_call(&json!({ "slug": "x" })), None);

        ended.tool_calls = vec![call(false, true)];
        assert_eq!(ExecutionReport::ending_call(&serde_json::to_value(&ended).unwrap()), None);
    }

    #[test]
    fn the_flag_is_only_serialised_when_set_and_defaults_when_absent() {
        let plain = serde_json::to_value(call(true, false)).unwrap();
        assert!(!plain.as_object().unwrap().contains_key("ends_turn"));

        let flagged = serde_json::to_value(call(true, true)).unwrap();
        assert_eq!(flagged["ends_turn"], true);

        let back: ToolInvocationRecord = serde_json::from_value(plain).unwrap();
        assert!(!back.ends_turn);
    }
}
