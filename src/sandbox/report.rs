//! The structured result of one `execute_python` run. It is the tool's output,
//! the persisted `ToolResult.content`, and the live WebUI event payload.

use std::time::Duration;

use monty_types::{ExcType, MontyException};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub ok: bool,
    /// JSON value of the script's last expression; `null` if none or on error.
    pub result: serde_json::Value,
    /// Captured `print()` output.
    pub stdout: String,
    pub error: Option<ExecutionError>,
    /// Ordered; empty when code mode is off.
    pub tool_calls: Vec<ToolInvocationRecord>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionError {
    pub kind: ExecutionErrorKind,
    /// e.g. "TypeError: unsupported operand type(s) for +: 'int' and 'str'"
    pub message: String,
    /// CPython-style traceback with line numbers; empty for host-raised limits.
    pub traceback: String,
    /// For `Limit`: "timeout" | "memory" | "recursion" | "script_size".
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
    pub error: Option<String>,
    pub duration_ms: u64,
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
