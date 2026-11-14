//! The authoring tools of `specs/061-agent-custom-tools` (contracts/agent-tools.md §2), category
//! `custom_tools`. Every refusal or miss is a `status` result, never an error; the work is done by
//! [`crate::custom_tools::CustomTools`], the write path people and dreams share.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    custom_tools::{CreateInput, CustomTools, Env, UpdateInput},
    error::VizierError,
};

fn failed(err: anyhow::Error) -> VizierError {
    VizierError(format!("custom tools: {err}"))
}

/// The names the agent can call from `ctx`, for reads (attention, model backing).
async fn names(ctx: &ToolContext, service: &CustomTools) -> Option<crate::custom_tools::ToolNames> {
    match &ctx.custom.script_router {
        Some(router) => Some(crate::custom_tools::router_names(router, ctx).await),
        None => service.published_names(&ctx.session.0).await,
    }
}

macro_rules! custom_tool {
    ($tool:ident, $name:literal) => {
        pub struct $tool {
            pub service: CustomTools,
        }

        impl $tool {
            pub fn new(service: CustomTools) -> Self {
                Self { service }
            }
        }

        impl $tool {
            #[allow(dead_code)]
            pub const NAME: &'static str = $name;
        }
    };
}

custom_tool!(CreateCustomTool, "create_custom_tool");
custom_tool!(UpdateCustomTool, "update_custom_tool");
custom_tool!(ListCustomTools, "list_custom_tools");
custom_tool!(GetCustomTool, "get_custom_tool");
custom_tool!(CustomToolHistory, "custom_tool_history");
custom_tool!(DiffCustomTool, "diff_custom_tool");
custom_tool!(RollbackCustomTool, "rollback_custom_tool");
custom_tool!(DeleteCustomTool, "delete_custom_tool");
custom_tool!(SetCustomToolEnabled, "set_custom_tool_enabled");
custom_tool!(UpdateCustomToolCategory, "update_custom_tool_category");

#[async_trait::async_trait]
impl VizierTool for CreateCustomTool {
    type Input = CreateInput;
    type Output = serde_json::Value;

    fn name() -> String {
        Self::NAME.into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::CustomTools)
    }

    fn description(&self) -> String {
        "Save a script as a custom tool of your own, called from any later script by its own name, <name>(**arguments); the name must not be one any of your tools already has. \
         The script gets its validated arguments as the dict `input` (defaults filled in) and the value of its last \
         expression is the result; it can call every tool an execute_python script can. Use a custom tool for a \
         workflow whose steps are fixed and only the inputs change; use a skill when the steps need judgment. \
         Give a category (one of yours, or a new one with category_description) so it is listed under it; with \
         sample_input the tool is run once for real before it is saved. Every problem comes back at once as \
         {status: \"invalid\", problems: [...]}; fix them all and save again."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let env = Env::from_context(ctx).await;
        self.service.create(&env, args).await.map_err(failed)
    }
}

#[async_trait::async_trait]
impl VizierTool for UpdateCustomTool {
    type Input = UpdateInput;
    type Output = serde_json::Value;

    fn name() -> String {
        Self::NAME.into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::CustomTools)
    }

    fn description(&self) -> String {
        "Change one of your custom tools: give its name and only the fields to change (new_name renames it, \
         category moves it). Every change is a new version; the previous ones stay in custom_tool_history. \
         Pass expected_version to be told out_of_date instead of overwriting a change you have not seen."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let env = Env::from_context(ctx).await;
        self.service.update(&env, args).await.map_err(failed)
    }
}

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct ListCustomToolsInput {
    #[serde(default)]
    #[schemars(description = "Only this category.")]
    pub category: Option<String>,
    #[serde(default = "yes")]
    #[schemars(description = "Include switched-off tools (default true).")]
    pub include_disabled: bool,
}

fn yes() -> bool {
    true
}

#[async_trait::async_trait]
impl VizierTool for ListCustomTools {
    type Input = ListCustomToolsInput;
    type Output = serde_json::Value;

    fn name() -> String {
        Self::NAME.into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::CustomTools)
    }

    fn description(&self) -> String {
        "List your custom tools (name, call_as, category, version, enabled, needs_attention, model_backed, author) \
         and your categories."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let names = names(ctx, &self.service).await;
        self.service
            .list(&ctx.session.0, names.as_ref(), args.category.as_deref(), args.include_disabled)
            .await
            .map_err(failed)
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NameInput {
    #[schemars(description = "The custom tool's name.")]
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetCustomToolInput {
    #[schemars(description = "The custom tool's name.")]
    pub name: String,
    #[serde(default)]
    #[schemars(description = "A past version; the current one when left out.")]
    pub version: Option<i64>,
}

#[async_trait::async_trait]
impl VizierTool for GetCustomTool {
    type Input = GetCustomToolInput;
    type Output = serde_json::Value;

    fn name() -> String {
        Self::NAME.into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::CustomTools)
    }

    fn description(&self) -> String {
        "Read one custom tool in full: description, input schema, script, the tools it calls, the custom tools that \
         call it, and whether it needs attention."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let names = names(ctx, &self.service).await;
        self.service
            .get(&ctx.session.0, names.as_ref(), &args.name, args.version)
            .await
            .map_err(failed)
    }
}

#[async_trait::async_trait]
impl VizierTool for CustomToolHistory {
    type Input = NameInput;
    type Output = serde_json::Value;

    fn name() -> String {
        Self::NAME.into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::CustomTools)
    }

    fn description(&self) -> String {
        "A custom tool's versions (newest first: who, why, when) and its events (switched on or off, category renamed)."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        self.service.history(&ctx.session.0, &args.name).await.map_err(failed)
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DiffCustomToolInput {
    #[schemars(description = "The custom tool's name.")]
    pub name: String,
    #[schemars(description = "The older version.")]
    pub from: i64,
    #[serde(default)]
    #[schemars(description = "The newer version; the current one when left out.")]
    pub to: Option<i64>,
}

#[async_trait::async_trait]
impl VizierTool for DiffCustomTool {
    type Input = DiffCustomToolInput;
    type Output = serde_json::Value;

    fn name() -> String {
        Self::NAME.into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::CustomTools)
    }

    fn description(&self) -> String {
        "Line diffs of a custom tool's description, input schema and script between two versions.".into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        self.service.diff(&ctx.session.0, &args.name, args.from, args.to).await.map_err(failed)
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RollbackCustomToolInput {
    #[schemars(description = "The custom tool's name.")]
    pub name: String,
    #[schemars(description = "The version to bring back; it is saved as a new version.")]
    pub to_version: i64,
    #[serde(default)]
    pub expected_version: Option<i64>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[async_trait::async_trait]
impl VizierTool for RollbackCustomTool {
    type Input = RollbackCustomToolInput;
    type Output = serde_json::Value;

    fn name() -> String {
        Self::NAME.into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::CustomTools)
    }

    fn description(&self) -> String {
        "Bring back an earlier version of a custom tool. It is checked like any change and saved as a new version.".into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let env = Env::from_context(ctx).await;
        self.service
            .rollback(&env, &args.name, args.to_version, args.expected_version, args.reason)
            .await
            .map_err(failed)
    }
}

#[async_trait::async_trait]
impl VizierTool for DeleteCustomTool {
    type Input = NameInput;
    type Output = serde_json::Value;

    fn name() -> String {
        Self::NAME.into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::CustomTools)
    }

    fn description(&self) -> String {
        "Delete one of your custom tools and its history. Refused while other custom tools call it.".into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let env = Env::from_context(ctx).await;
        self.service.delete(&env, &args.name).await.map_err(failed)
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SetCustomToolEnabledInput {
    #[schemars(description = "The custom tool's name.")]
    pub name: String,
    #[schemars(description = "true to offer it again, false to switch it off.")]
    pub enabled: bool,
}

#[async_trait::async_trait]
impl VizierTool for SetCustomToolEnabled {
    type Input = SetCustomToolEnabledInput;
    type Output = serde_json::Value;

    fn name() -> String {
        Self::NAME.into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::CustomTools)
    }

    fn description(&self) -> String {
        "Switch one of your custom tools off or back on. A tool a person switched off only a person can switch on.".into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let env = Env::from_context(ctx).await;
        self.service.set_enabled(&env, &args.name, args.enabled).await.map_err(failed)
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UpdateCustomToolCategoryInput {
    #[schemars(description = "The category to change.")]
    pub name: String,
    #[serde(default)]
    #[schemars(description = "A new one-line description.")]
    pub description: Option<String>,
    #[serde(default)]
    #[schemars(description = "A new name; every tool in it moves with it and keeps its name.")]
    pub new_name: Option<String>,
}

#[async_trait::async_trait]
impl VizierTool for UpdateCustomToolCategory {
    type Input = UpdateCustomToolCategoryInput;
    type Output = serde_json::Value;

    fn name() -> String {
        Self::NAME.into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::CustomTools)
    }

    fn description(&self) -> String {
        "Rename or redescribe one of your custom tool categories. The change shows in your category list from your \
         next turn. To move a single tool, use update_custom_tool with category."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let env = Env::from_context(ctx).await;
        self.service
            .update_category(&env, &args.name, args.description.as_deref(), args.new_name.as_deref())
            .await
            .map_err(failed)
    }
}

#[cfg(test)]
#[path = "custom_tools_tests.rs"]
mod tests;
