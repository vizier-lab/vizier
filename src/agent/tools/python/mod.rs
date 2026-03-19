use std::{ffi::CString, fs, path::PathBuf, str::FromStr, sync::Arc};

use pyo3::{prelude::*, types::PyDict};
use rig::{completion::ToolDefinition, tool::Tool};
use schemars::schema_for;
use serde::{Deserialize, Serialize};

use crate::error::{VizierError, throw_vizier_error};

mod docs;
mod ptc;

pub use ptc::ProgrammaticToolCall;

pub struct PythonInterpreter {
    workdir: String,
    builtins_whitelist: Vec<String>,
    import_whitelist: Vec<String>,
    programmatic_tools: Vec<Box<dyn ProgrammaticToolCall>>,
}

impl PythonInterpreter {
    pub fn new(workdir: String) -> Self {
        fs::create_dir_all(PathBuf::from(workdir.clone())).unwrap();

        let builtins_whitelist = vec![
            // Existing
            "print".to_string(),
            "len".to_string(),
            "int".to_string(),
            "str".to_string(),
            "float".to_string(),
            "list".to_string(),
            "dict".to_string(),
            "range".to_string(),
            "sum".to_string(),
            "min".to_string(),
            "max".to_string(),
            "tuple".to_string(),
            // Highly Recommended
            "bool".to_string(),
            "set".to_string(),
            "bytes".to_string(),
            "enumerate".to_string(),
            "zip".to_string(),
            "sorted".to_string(),
            "reversed".to_string(),
            "any".to_string(),
            "all".to_string(),
            "isinstance".to_string(),
            "issubclass".to_string(),
            "type".to_string(),
            "abs".to_string(),
            "round".to_string(),
            "pow".to_string(),
            "map".to_string(),
            "filter".to_string(),
            "getattr".to_string(),
            "hasattr".to_string(),
            "setattr".to_string(),
            "repr".to_string(),
            "dir".to_string(),
            "staticmethod".to_string(),
            "classmethod".to_string(),
            "property".to_string(),
            // --- Add these for error handling ---
            "Exception".to_string(),
            "ValueError".to_string(),
            "TypeError".to_string(),
            "KeyError".to_string(),
            "IndexError".to_string(),
            "StopIteration".to_string(),
            "RuntimeError".to_string(),
            "open".to_string(),
        ];

        let import_whitelist = vec![
            "math".to_string(),
            "json".to_string(),
            "cmath".to_string(),
            "datetime".to_string(),
            "time".to_string(),
            "re".to_string(),
            "collections".to_string(),
            "itertools".to_string(),
            "functools".to_string(),
            "statistic".to_string(),
            "string".to_string(),
        ];

        Self {
            workdir,
            builtins_whitelist,
            import_whitelist,
            programmatic_tools: Vec::new(),
        }
    }

    /// Register a tool to be callable from Python scripts (builder pattern)
    pub fn with_tool(mut self, tool: Box<dyn ProgrammaticToolCall>) -> Self {
        self.programmatic_tools.push(tool);
        self
    }

    /// Register a tool using Arc wrapper (builder pattern)
    pub fn tool<T>(mut self, tool: T) -> Self
    where
        T: Tool<Error = VizierError> + Send + Sync + 'static,
        T::Args: for<'de> Deserialize<'de> + schemars::JsonSchema + Send,
        T::Output: Serialize + schemars::JsonSchema,
    {
        self.programmatic_tools.push(Box::new(Arc::new(tool)));
        self
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PythonInterpreterArgs {
    #[schemars(description = "Python script to run")]
    pub script: String,
}

#[pyclass]
struct PythonInterpreterContext {
    #[pyo3(get)]
    allowed_modules: Vec<String>,
    /// The original __import__ function, cached to avoid recursion
    real_import: Py<PyAny>,
}

#[pymethods]
impl PythonInterpreterContext {
    #[new]
    fn new(allowed: Vec<String>, real_import: Py<PyAny>) -> Self {
        Self {
            allowed_modules: allowed,
            real_import,
        }
    }
}

#[pyfunction]
#[pyo3(signature = (name, globals=None, locals=None, fromlist=None, level=0, *, ctx))]
fn dynamic_import(
    py: Python<'_>,
    name: String,
    globals: Option<Bound<'_, PyAny>>,
    locals: Option<Bound<'_, PyAny>>,
    fromlist: Option<Bound<'_, PyAny>>,
    level: i32,
    ctx: Py<PythonInterpreterContext>,
) -> PyResult<Py<PyAny>> {
    let root_module = name.split('.').next().unwrap_or("");

    let ctx_ref = ctx.bind(py).borrow();
    let is_allowed =
        root_module == "builtins" || ctx_ref.allowed_modules.contains(&root_module.to_string());

    if is_allowed {
        // Use the cached real __import__ to avoid recursion.
        // Previously we called py.import("builtins") here, which would trigger
        // our custom import handler, causing infinite recursion -> stack overflow.
        let real_import = ctx_ref.real_import.bind(py);
        let result = real_import.call1((name, globals, locals, fromlist, level))?;
        Ok(result.unbind())
    } else {
        Err(pyo3::exceptions::PyImportError::new_err(format!(
            "Vizier Security Policy Blocked: '{}' is not in the allowed whitelist.",
            root_module
        )))
    }
}

impl Tool for PythonInterpreter
where
    Self: Send + Sync,
{
    const NAME: &'static str = "python_interpreter";
    type Error = VizierError;
    type Args = PythonInterpreterArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let parameters = serde_json::to_value(schema_for!(Self::Args)).unwrap();

        // Build available tools description
        let tools_desc = if self.programmatic_tools.is_empty() {
            String::new()
        } else {
            let mut tool_docs = Vec::new();
            for tool in &self.programmatic_tools {
                tool_docs.push(format!("\n--- {} ---\n{}", tool.name(), tool.describe()));
            }
            format!(
                "\n\nAvailable programmatic tools (callable as functions with kwargs):\n{}",
                tool_docs.join("\n")
            )
        };

        let description = format!(
            "Run a Python script in a sandboxed environment.\n\n\
            **only use this tools for calculation and accessing tools**
            Allowed builtins: {}\n\n\
            Allowed imports: {}{}",
            self.builtins_whitelist.join(", "),
            self.import_whitelist.join(", "),
            tools_desc
        );

        ToolDefinition {
            name: Self::NAME.to_string(),
            description,
            parameters,
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        log::info!("python_interpreter {}", args.script.clone());

        let res = Python::attach(|py| -> PyResult<String> {
            let os = py.import("os")?;
            let sys = py.import("sys")?;

            let old_cwd: String = os.call_method0("getcwd")?.extract()?;
            os.call_method1("chdir", (self.workdir.clone(),))?;

            let run_result = (|| -> PyResult<String> {
                let globals = PyDict::new(py);
                let builtins = py.import("builtins")?;

                let restricted_builtins = PyDict::new(py);

                for func_name in &self.builtins_whitelist {
                    if let Ok(func) = builtins.getattr(func_name.as_str()) {
                        restricted_builtins.set_item(func_name, func)?;
                    }
                }

                // Cache the real __import__ BEFORE we override it.
                // This prevents infinite recursion in dynamic_import.
                let real_import = builtins.getattr("__import__")?.unbind();

                let ctx = Bound::new(
                    py,
                    PythonInterpreterContext::new(self.import_whitelist.clone(), real_import),
                )?;

                let import_handler = wrap_pyfunction!(dynamic_import, py)?;

                globals.set_item("__v_imp__", &import_handler)?;
                globals.set_item("ctx", &ctx)?;

                let guarded_import = py.eval(
                    // We use eval here because we want the lambda function object returned
                    c"lambda name, globals=None, locals=None, fromlist=None, level=0: __v_imp__(name, globals, locals, fromlist, level, ctx=ctx)",
                    Some(&globals),
                    None,
                )?;

                restricted_builtins.set_item("__import__", guarded_import)?;

                globals.set_item("__builtins__", restricted_builtins)?;

                // Register programmatic tools as callable functions
                for tool in &self.programmatic_tools {
                    tool.register_in_globals(py, &globals)?;
                }

                let capture = Py::new(py, OutputCapture::new())?;
                sys.setattr("stdout", &capture)?;

                py.run(
                    CString::from_str(&args.script).unwrap().as_c_str(),
                    Some(&globals),
                    None,
                )?;

                let capture_borrow = capture.borrow(py);
                let output = capture_borrow.data.join("\n");

                Ok(output)
            })();

            os.call_method1("chdir", (old_cwd,))?;
            run_result
        });

        match res {
            Ok(output) => Ok(output),
            Err(err) => throw_vizier_error("python_interpreter", err),
        }
    }
}

#[pyclass]
struct OutputCapture {
    pub data: Vec<String>,
}

#[pymethods]
impl OutputCapture {
    #[new]
    fn new() -> Self {
        OutputCapture { data: Vec::new() }
    }

    fn write(&mut self, data: String) {
        self.data.push(data);
    }

    fn flush(&self) {} // Python expects a flush method
}
