use std::sync::{Arc, Mutex};

use rustpython;
use rustpython_vm::{self as vm};
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;

use crate::{
    agents::tools::{VizierTool, VizierTools},
    error::VizierError,
};

pub struct ProgramaticSandbox {
    pub tools: Arc<VizierTools>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ProgramaticSandboxArgs {
    #[schemars(description = "script to run")]
    pub script: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ProgramaticSandboxOutput {
    #[schemars(description = "console_output")]
    pub console_outputs: String,
}

#[async_trait::async_trait]
impl VizierTool for ProgramaticSandbox {
    type Input = ProgramaticSandboxArgs;
    type Output = ProgramaticSandboxOutput;

    fn name() -> String {
        "programmatic_sandbox".to_string()
    }

    fn description(&self) -> String {
        r#"Run a Python script in a sandboxed environment.

Available functions:
- print(str): Print values to output, in this sandbox you can't do print() without any args, you need to use this to get or format the result of tool_call from console output
- tool_call(function_name, args_json): Call external tools. Returns JSON string based on output schema.

Examples:
  tool_call("web_search", "{ \"query\": \"some query\", \"page\": 1 }")
  print("some str")

All tool_call results are serialized as JSON strings matching the output schema."#
        .into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let script = args.script.clone();
        let tools = self.tools.clone();

        let console = Arc::new(Mutex::new(vec![]));

        let vm_console = console.clone();
        let interpreter = rustpython::InterpreterConfig::new()
            .init_stdlib()
            .interpreter();

        interpreter.enter(|vm| {
            let scope: vm::scope::Scope = vm.new_scope_with_builtins();
            let tools = tools.clone();
            let print = vm.new_function("print", move |str: String| {
                vm_console.lock().unwrap().push(str);
            });

            let tool_call =
                vm.new_function("tool_call", move |function_name: String, params: String| {
                    let tool_call = tools.call(function_name, params);
                    let handle = Handle::try_current().unwrap();
                    // We're inside a tokio runtime, use block_in_place
                    let result = tokio::task::block_in_place(|| {
                        handle.block_on(async { tool_call.await }).unwrap()
                    });

                    return result;
                });

            let _ = scope.globals.set_item("print", print.into(), vm);
            let _ = scope.globals.set_item("tool_call", tool_call.into(), vm);

            let code_obj = vm
                .compile(&script, vm::compiler::Mode::Exec, "<embedded>".to_owned())
                .map_err(|err| vm.new_syntax_error(&err, Some(&script)))
                .unwrap();

            let _ = vm.run_code_obj(code_obj, scope.clone());
        });

        drop(interpreter);

        Ok(ProgramaticSandboxOutput {
            console_outputs: console.lock().unwrap().join("\n"),
        })
    }
}
