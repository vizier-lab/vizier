use rig::{completion::ToolDefinition, tool::Tool};
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use std::process::Command;

use crate::error::{VizierError, throw_vizier_error};

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ExecCliArgs {
    #[schemars(description = "CLI command to execute")]
    pub commands: String,
}

pub struct ExecCliFromWorkspace(pub String);

impl Tool for ExecCliFromWorkspace {
    const NAME: &'static str = "exec_cli_from_workspace";
    type Error = VizierError;
    type Args = ExecCliArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let parameters = serde_json::to_value(schema_for!(Self::Args)).unwrap();

        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "run a a CLI command on a workspace directory".into(),
            parameters,
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        log::info!("exec_cli_from_workspace: {}", args.commands.clone());
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", &args.commands])
                .current_dir(self.0.clone())
                .output()
        } else {
            Command::new("sh")
                .arg("-c")
                .args([&args.commands])
                .current_dir(self.0.clone())
                .output()
        };

        match output {
            Err(err) => throw_vizier_error("cli command", err),
            Ok(output) => Ok(String::from_utf8(output.stdout).unwrap()),
        }
    }
}
