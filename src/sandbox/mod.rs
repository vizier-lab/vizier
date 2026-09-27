//! Sandboxed Python execution on top of the `monty` interpreter.
//!
//! Engine-facing only: this module knows nothing about agents, `VizierTools` or
//! storage. The agent layer plugs its tools in through [`SandboxBridge`].

use std::time::Duration;

pub mod convert;
pub mod docs;
pub mod report;
pub mod runtime;

pub use docs::ToolFunctionDoc;
pub use report::{ExecutionError, ExecutionErrorKind, ExecutionReport, ToolInvocationRecord};
pub use runtime::execute;

/// Monty's per-operation allocation pre-check. A guard against a single huge
/// allocation (`"a" * 10**10`), not a cumulative memory ceiling.
pub const SINGLE_ALLOCATION_GUARD: usize = 1 << 30;

/// Scripts larger than this are refused before the interpreter starts.
pub const MAX_SCRIPT_BYTES: usize = 64 * 1024;

/// What the engine is handed per run.
#[derive(Debug, Clone, Copy)]
pub struct SandboxLimits {
    /// The agent's `tools.timeout`; becomes Monty's CPU clock limit.
    pub timeout: Duration,
    /// Whether scripts may call the agent's tools (code mode).
    pub tools_enabled: bool,
}

/// Why a bridged call produced no value.
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeError {
    /// No such function: the script sees `NameError`.
    UnknownFunction,
    /// The tool ran and failed: the script sees a catchable `RuntimeError`.
    Tool(String),
}

/// Implemented by the agent layer; the engine never sees `VizierTools`.
#[async_trait::async_trait]
pub trait SandboxBridge: Send + Sync {
    /// Catalogue for `list_tools()`; also used to resolve names.
    async fn catalogue(&self) -> Vec<ToolFunctionDoc>;

    async fn describe(&self, function: &str) -> Option<ToolFunctionDoc>;

    /// Invoke a tool with the JSON object built from the script's call.
    async fn call(
        &self,
        function: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, BridgeError>;
}

/// Bridge for runs without tool access (sandbox-only mode, unit tests).
pub struct NoToolsBridge;

#[async_trait::async_trait]
impl SandboxBridge for NoToolsBridge {
    async fn catalogue(&self) -> Vec<ToolFunctionDoc> {
        vec![]
    }

    async fn describe(&self, _function: &str) -> Option<ToolFunctionDoc> {
        None
    }

    async fn call(
        &self,
        _function: &str,
        _arguments: serde_json::Value,
    ) -> Result<serde_json::Value, BridgeError> {
        Err(BridgeError::UnknownFunction)
    }
}
