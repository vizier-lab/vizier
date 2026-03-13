use std::{ffi::CString, fs, path::PathBuf, str::FromStr};

use pyo3::{prelude::*, types::PyDict};
use rig::{completion::ToolDefinition, tool::Tool};
use schemars::schema_for;
use serde::{Deserialize, Serialize};

use crate::error::{VizierError, throw_vizier_error};

pub struct PythonInterpreter {
    workdir: String,
}

impl PythonInterpreter {
    pub fn new(workdir: String) -> Self {
        fs::create_dir_all(PathBuf::from(workdir.clone())).unwrap();

        Self { workdir }
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
}

#[pymethods]
impl PythonInterpreterContext {
    #[new]
    fn new(allowed: Vec<String>) -> Self {
        Self {
            allowed_modules: allowed,
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
    // 1. Extract the root module (e.g., "os" from "os.path")
    let root_module = name.split('.').next().unwrap_or("");

    // 2. Check whitelist against the root module
    let is_allowed = ctx
        .bind(py)
        .borrow()
        .allowed_modules
        .contains(&root_module.to_string());

    if is_allowed {
        let builtins = py.import("builtins")?;
        let real_import = builtins.getattr("__import__")?;

        // 3. Call the real internal Python __import__
        let result = real_import.call1((name, globals, locals, fromlist, level))?;

        Ok(result.unbind())
    } else {
        // Explicitly block unauthorized access
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

        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "run a python script, could be use to interact with your environment"
                .to_string(),
            parameters,
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        log::info!("python_interpreter {}", args.script.clone());

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

        let builtins_whitelist = [
            // Existing
            "print",
            "len",
            "int",
            "str",
            "float",
            "list",
            "dict",
            "range",
            "sum",
            "min",
            "max",
            "tuple",
            // Highly Recommended
            "bool",
            "set",
            "tuple",
            "bytes",
            "enumerate",
            "zip",
            "sorted",
            "reversed",
            "any",
            "all",
            "isinstance",
            "issubclass",
            "type",
            "abs",
            "round",
            "pow",
            "map",
            "filter",
            "getattr",
            "hasattr",
            "setattr",
            "repr",
            "dir",
            "staticmethod",
            "classmethod",
            "property",
            // --- Add these for error handling ---
            "Exception",
            "ValueError",
            "TypeError",
            "KeyError",
            "IndexError",
            "StopIteration",
            "RuntimeError",
            "open",
        ];

        let res = Python::attach(|py| -> PyResult<String> {
            let os = py.import("os")?;
            let sys = py.import("sys")?;

            let old_cwd: String = os.call_method0("getcwd")?.extract()?;
            os.call_method1("chdir", (self.workdir.clone(),))?;

            let run_result = (|| -> PyResult<String> {
                let globals = PyDict::new(py);
                let builtins = py.import("builtins")?;

                let restricted_builtins = PyDict::new(py);

                for func_name in builtins_whitelist {
                    if let Ok(func) = builtins.getattr(func_name) {
                        restricted_builtins.set_item(func_name, func)?;
                    }
                }

                let ctx = Bound::new(py, PythonInterpreterContext::new(import_whitelist))?;

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
