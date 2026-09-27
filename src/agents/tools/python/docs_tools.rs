//! `list_tool_functions` / `describe_tool_function`: the model-side twins of the
//! in-script `list_tools()` / `describe_tool()`, sharing the same `docs` code.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolContext, ToolRouter, VizierTool},
    error::VizierError,
    sandbox::docs,
};

/// Output schema of the tool behind a function, when it is a native tool.
pub(super) fn output_schema(router: &ToolRouter, tool: &str) -> Option<serde_json::Value> {
    router
        .default_toolset
        .get_tool(tool.to_string())
        .or_else(|_| router.user_toolset.get_tool(tool.to_string()))
        .ok()
        .map(|tool| tool.output_schema())
}

pub struct ListToolFunctions {
    router: ToolRouter,
}

impl ListToolFunctions {
    pub fn new(router: ToolRouter) -> Self {
        Self { router }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListToolFunctionsInput {}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ToolFunctionSummary {
    pub function: String,
    pub summary: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListToolFunctionsOutput {
    pub count: usize,
    pub functions: Vec<ToolFunctionSummary>,
}

#[async_trait::async_trait]
impl VizierTool for ListToolFunctions {
    type Input = ListToolFunctionsInput;
    type Output = ListToolFunctionsOutput;

    fn name() -> String {
        "list_tool_functions".into()
    }

    fn description(&self) -> String {
        "List every function this agent can call from an execute_python script, with a one-line summary each. Use describe_tool_function for parameters and examples.".into()
    }

    async fn call(&self, _args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let defs = self
            .router
            .definitions()
            .await
            .map_err(|err| VizierError(err.to_string()))?;
        let functions: Vec<ToolFunctionSummary> = docs::catalogue(&defs)
            .into_iter()
            .map(|doc| ToolFunctionSummary {
                function: doc.function,
                summary: doc.summary,
            })
            .collect();

        Ok(ListToolFunctionsOutput {
            count: functions.len(),
            functions,
        })
    }
}

pub struct DescribeToolFunction {
    router: ToolRouter,
}

impl DescribeToolFunction {
    pub fn new(router: ToolRouter) -> Self {
        Self { router }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DescribeToolFunctionInput {
    #[schemars(description = "Function name as shown by list_tool_functions")]
    pub name: String,
}

#[async_trait::async_trait]
impl VizierTool for DescribeToolFunction {
    type Input = DescribeToolFunctionInput;
    /// A `ToolFunctionDoc`, or `{available: false, name, message, did_you_mean}`.
    type Output = serde_json::Value;

    fn name() -> String {
        "describe_tool_function".into()
    }

    fn description(&self) -> String {
        "Show the parameters, return shape and a usage example for one function callable from execute_python.".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let defs = self
            .router
            .definitions()
            .await
            .map_err(|err| VizierError(err.to_string()))?;
        let catalogue = docs::catalogue(&defs);

        let found = catalogue
            .iter()
            .find(|doc| doc.function == args.name)
            .and_then(|doc| {
                docs::describe_in(&defs, &doc.function, output_schema(&self.router, &doc.tool).as_ref())
            });

        Ok(match found {
            Some(doc) => serde_json::to_value(doc).map_err(|err| VizierError(err.to_string()))?,
            None => {
                let known: Vec<String> = catalogue.into_iter().map(|doc| doc.function).collect();
                docs::not_found(&args.name, &known)
            }
        })
    }
}
