use serde::{Deserialize, Serialize};

use crate::{agents::tools::VizierTool, error::VizierError};

pub struct ThinkTool;

#[async_trait::async_trait]
impl VizierTool for ThinkTool {
    type Input = ThinkToolArgs;
    type Output = String;

    fn name() -> String {
        "think".into()
    }

    fn description(&self) -> String {
        "Use the tool to think about something. It will not obtain new information
            or change the database, but just append the thought to the log. Use it when complex
            reasoning or some cache memory is needed."
            .to_string()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        Ok(args.thought)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ThinkToolArgs {
    #[schemars(description = "A thought to think about.")]
    thought: String,
}
