use std::sync::{Arc, Mutex};

use rustpython;
use rustpython_vm::{self as vm, VirtualMachine};
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;

use crate::{
    agents::tools::{VizierTool, VizierToolSet, ptc::converter::json_to_py},
    error::VizierError,
};

mod converter;

pub struct ProgramaticSandbox {
    pub tools: Arc<VizierToolSet>,
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
        let available_tools = self
            .tools
            .tools
            .iter()
            .map(|(function_name, tool)| {
                format!(
                    "tool_name: {}\ndescription: {}\ninput: {}\noutput: {}\n---",
                    function_name,
                    tool.description(),
                    tool.input_schema(),
                    tool.output_schema()
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        let examples = r#"tool_call("web_search", "{ \"query\": \"some query\", \"page\": 1 }")"#;

        format!(
            r#"Run a Python script in a sandboxed environment.

Available functions:
- output(str): Print string (and only accept string) to output, you need to use this to get or format the result of tool_call from console output, **do not use print()**
- tool_call(function_name, args_json): Call external tools. Returns python value (not a json string) based on output schema.

Examples:
  {examples}
  output("some str")

Available Tools ():
{}


All tool_call results are serialized as JSON strings matching the output schema."#,
            available_tools
        )
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
                println!(">> {str}");
                vm_console.lock().unwrap().push(str);
            });

            let tool_call = vm.new_function(
                "tool_call",
                move |function_name: String, params: String, vm: &VirtualMachine| {
                    let tool_call = tools.call(function_name, params);
                    let handle = Handle::try_current().unwrap();
                    // // We're inside a tokio runtime, use block_in_place
                    let result = tokio::task::block_in_place(|| {
                        match handle.block_on(async { tool_call.await }) {
                            Ok(val) => val,
                            Err(err) => serde_json::Value::String(err.to_string()),
                        }
                    });

                    return json_to_py(&result, &vm);
                },
            );

            let _ = scope.globals.set_item("output", print.into(), vm);
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
