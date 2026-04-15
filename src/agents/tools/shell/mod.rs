use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    agents::tools::VizierTool,
    error::VizierError,
    shell::{ShellProvider, VizierShell},
};

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ShellExecArgs {
    #[schemars(description = "shell command to execute")]
    pub commands: String,
}

pub struct ShellExec(pub Arc<VizierShell>);

#[async_trait::async_trait]
impl VizierTool for ShellExec {
    type Input = ShellExecArgs;
    type Output = String;

    fn name() -> String {
        "shell_exec".to_string()
    }

    fn description(&self) -> String {
        "run a a CLI command on a workspace directory".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        Ok(self
            .0
            .exec(args.commands)
            .map_err(|err| VizierError(err.to_string()))
            .await?)
    }
}
