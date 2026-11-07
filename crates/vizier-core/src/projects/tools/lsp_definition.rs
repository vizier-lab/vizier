//! `lsp_definition`: where a symbol is defined, by meaning rather than by text.

use super::lsp_common::{PositionInput, position_request, project};
use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    projects::lsp::query,
};

pub struct LspDefinition;

#[async_trait::async_trait]
impl VizierTool for LspDefinition {
    type Input = PositionInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "lsp_definition".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn description(&self) -> String {
        "Find where a symbol is defined, using the language server: the definition of the \
         function, type or variable named `symbol` on `line` of `path`, with no column needed. \
         Prefer this to `grep` when the name is common. A location outside the project comes \
         back as `outside_project`. When the server is still starting the result is \
         `{status: \"lsp_starting\"}`; try again later."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let project = project(ctx)?;
        Ok(
            match position_request(ctx, &args, "textDocument/definition", None).await {
                Ok((_ready, answer, enc)) => {
                    let locations = query::map_locations(&project.cwd, enc, &answer);
                    serde_json::json!({ "status": "ok", "locations": locations })
                }
                Err(status) => status,
            },
        )
    }
}
