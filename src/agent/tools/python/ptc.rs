use std::sync::Arc;

use pyo3::{
    prelude::*,
    types::{PyCFunction, PyDict},
};
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;

use crate::error::VizierError;

/// Trait for tools that can be called programmatically from Python
pub trait ProgrammaticToolCall: Send + Sync {
    /// Returns the tool name for registration in Python globals
    fn name(&self) -> &'static str;

    /// Returns a description of the tool and its arguments for LLM discovery
    fn describe(&self) -> String;

    /// Registers this tool as a callable function in the Python globals dict
    fn register_in_globals(&self, py: Python<'_>, globals: &Bound<'_, PyDict>) -> PyResult<()>;
}

impl<T> ProgrammaticToolCall for Arc<T>
where
    T: Tool<Error = VizierError> + Send + Sync + 'static,
    T::Args: for<'de> Deserialize<'de> + schemars::JsonSchema + Send,
    T::Output: Serialize + schemars::JsonSchema,
{
    fn name(&self) -> &'static str {
        T::NAME
    }

    fn describe(&self) -> String {
        let args_schema = schemars::schema_for!(T::Args);
        let args_schema_json = serde_json::to_string_pretty(&args_schema).unwrap_or_default();
        let output_schema = schemars::schema_for!(T::Output);
        let output_schema_json = serde_json::to_string_pretty(&output_schema).unwrap_or_default();
        format!(
            "Function: {}\nArguments (JSON Schema):\n{}\nOutput (JSON Schema):\n{}",
            T::NAME,
            args_schema_json,
            output_schema_json
        )
    }

    fn register_in_globals(&self, py: Python<'_>, globals: &Bound<'_, PyDict>) -> PyResult<()> {
        let tool = self.clone();

        // Create a Python closure that calls the tool
        let closure = PyCFunction::new_closure(
            py,
            Some(c"tool_call"),
            Some(c"Calls the underlying programmatic tool with given kwargs"),
            move |args: &Bound<'_, pyo3::types::PyTuple>,
                  kwargs: Option<&Bound<'_, PyDict>>|
                  -> PyResult<Py<PyAny>> {
                let py = args.py();

                // Convert kwargs to JSON value
                let json_args: serde_json::Value = if let Some(kw) = kwargs {
                    pythonize::depythonize(kw).map_err(|e| {
                        pyo3::exceptions::PyValueError::new_err(format!(
                            "Failed to parse arguments: {}",
                            e
                        ))
                    })?
                } else {
                    serde_json::json!({})
                };

                // Deserialize into the tool's Args type
                let tool_args: T::Args = serde_json::from_value(json_args).map_err(|e| {
                    pyo3::exceptions::PyValueError::new_err(format!("Invalid arguments: {}", e))
                })?;

                // Clone tool for async block
                let tool_clone = tool.clone();

                // Execute the async tool call using tokio
                let result = if let Ok(handle) = Handle::try_current() {
                    // We're inside a tokio runtime, use block_in_place
                    tokio::task::block_in_place(|| {
                        handle.block_on(async { tool_clone.call(tool_args).await })
                    })
                } else {
                    // No runtime, create a new one
                    tokio::runtime::Runtime::new()
                        .unwrap()
                        .block_on(async { tool_clone.call(tool_args).await })
                };

                // Convert result back to Python
                match result {
                    Ok(output) => {
                        let json_output = serde_json::to_value(&output).map_err(|e| {
                            pyo3::exceptions::PyRuntimeError::new_err(format!(
                                "Failed to serialize output: {}",
                                e
                            ))
                        })?;

                        let py_obj: Bound<'_, PyAny> = pythonize::pythonize(py, &json_output)
                            .map_err(|e| {
                                pyo3::exceptions::PyRuntimeError::new_err(format!(
                                    "Failed to convert to Python: {}",
                                    e
                                ))
                            })?;

                        Ok(py_obj.unbind())
                    }
                    Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                        "Tool execution failed: {:?}",
                        e
                    ))),
                }
            },
        )?;

        globals.set_item(T::NAME, closure)?;
        Ok(())
    }
}
