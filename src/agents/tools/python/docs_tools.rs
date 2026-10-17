//! `list_tool_functions` / `describe_tool_function`: the model-side twins of the
//! in-script `list_tools()` / `describe_tool()`, sharing the same `docs` code.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{
        CatalogueEntry, CategorySummary, ToolCategory, ToolContext, ToolRouter, ToolTarget,
        VizierTool, mcp_category_name, turn_catalogue, turn_categories,
    },
    error::VizierError,
    sandbox::{ToolFunctionDoc, docs},
};

/// The full documentation of one catalogue entry, output schema included for native tools
/// and for client tools that declared one.
pub(super) fn entry_doc(router: &ToolRouter, ctx: &ToolContext, entry: &CatalogueEntry) -> ToolFunctionDoc {
    let output_schema = match &entry.target {
        ToolTarget::Native(tool) => router
            .default_toolset
            .get_tool(tool.clone())
            .or_else(|_| router.user_toolset.get_tool(tool.clone()))
            .ok()
            .map(|tool| tool.output_schema()),
        ToolTarget::Mcp { .. } => None,
        ToolTarget::Client { .. } => ctx
            .client
            .as_ref()
            .and_then(|client| client.declaration(&entry.name))
            .and_then(|declaration| declaration.output_schema.clone()),
    };
    docs::describe_as(
        entry.name.clone(),
        &entry.definition,
        output_schema.as_ref(),
        Some(entry.category.clone()),
    )
}

pub struct ListToolFunctions {
    router: ToolRouter,
    categories: Vec<CategorySummary>,
}

impl ListToolFunctions {
    pub fn new(router: ToolRouter, categories: Vec<CategorySummary>) -> Self {
        Self { router, categories }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListToolFunctionsInput {
    #[serde(default)]
    #[schemars(
        description = "A category from the system prompt's list, e.g. \"memory\". Leave it out to list every function with its category."
    )]
    pub category: Option<String>,
}

#[async_trait::async_trait]
impl VizierTool for ListToolFunctions {
    type Input = ListToolFunctionsInput;
    /// `{count, functions}`, `{category, count, functions}`, or
    /// `{available: false, category, message, did_you_mean}`.
    type Output = serde_json::Value;

    fn name() -> String {
        "list_tool_functions".into()
    }

    fn category() -> Option<ToolCategory> {
        None
    }

    fn description(&self) -> String {
        "List the functions this agent can call from an execute_python script. Pass a `category` from the system prompt's list to see only that category's functions, each {\"name\", \"summary\"}; leave it out to list every function with its category. Pass a name to describe_tool_function for parameters and examples.".into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let catalogue = turn_catalogue(&self.router, ctx).await;
        let categories = turn_categories(&self.categories, ctx);

        let Some(category) = args.category else {
            let functions: Vec<serde_json::Value> = catalogue
                .iter()
                .map(|entry| {
                    serde_json::json!({
                        "name": entry.name,
                        "summary": docs::summary(&entry.definition.description),
                        "category": entry.category,
                    })
                })
                .collect();
            return Ok(serde_json::json!({ "count": functions.len(), "functions": functions }));
        };

        if !categories.iter().any(|known| known.name == category) {
            let known: Vec<String> = categories.iter().map(|known| known.name.clone()).collect();
            return Ok(serde_json::json!({
                "available": false,
                "category": category,
                "message": format!("No category named '{category}'."),
                "did_you_mean": docs::did_you_mean(&category, &known),
            }));
        }

        let functions: Vec<serde_json::Value> = catalogue
            .iter()
            .filter(|entry| entry.category == category)
            .map(|entry| {
                serde_json::json!({
                    "name": entry.name,
                    "summary": docs::summary(&entry.definition.description),
                })
            })
            .collect();
        let mut output = serde_json::json!({
            "category": category,
            "count": functions.len(),
            "functions": functions,
        });

        let server = self
            .router
            .mcp
            .keys()
            .find(|server| mcp_category_name(server, &self.router.reserved) == category);
        if let (true, Some(server)) = (functions.is_empty(), server) {
            output["message"] = serde_json::json!(format!(
                "No functions from the {server} MCP server right now; it may be unreachable. Try again later."
            ));
        }
        Ok(output)
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

    fn category() -> Option<ToolCategory> {
        None
    }

    fn description(&self) -> String {
        "Show the parameters, return shape and a usage example for one function callable from execute_python.".into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let catalogue = turn_catalogue(&self.router, ctx).await;

        Ok(match catalogue.iter().find(|entry| entry.name == args.name) {
            Some(entry) => serde_json::to_value(entry_doc(&self.router, ctx, entry))
                .map_err(|err| VizierError(err.to_string()))?,
            None => {
                let known: Vec<String> = catalogue.into_iter().map(|entry| entry.name).collect();
                docs::not_found(&args.name, &known)
            }
        })
    }
}
