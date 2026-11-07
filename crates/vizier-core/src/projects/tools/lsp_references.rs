//! `lsp_references`: every real use of a symbol, not every occurrence of its name.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::lsp_common::{PositionInput, position_request, project};
use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    projects::lsp::query,
};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct LspReferencesInput {
    #[serde(flatten)]
    pub position: PositionInput,
    /// Also return the declaration itself (default false).
    #[serde(default)]
    pub include_declaration: Option<bool>,
}

pub struct LspReferences;

#[async_trait::async_trait]
impl VizierTool for LspReferences {
    type Input = LspReferencesInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "lsp_references".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn description(&self) -> String {
        "Find every use of a symbol with the language server, so a same-named function elsewhere \
         is not counted. Name the symbol and the line it is on. Results are cut at 50 with \
         `truncated: true` and a hint on narrowing the question. Prefer this to `grep` before \
         renaming or changing a signature."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let project = project(ctx)?;
        let extra = serde_json::json!({
            "context": { "includeDeclaration": args.include_declaration.unwrap_or(false) }
        });
        Ok(
            match position_request(ctx, &args.position, "textDocument/references", Some(extra)).await {
                Ok((_ready, answer, enc)) => {
                    let all = query::map_locations(&project.cwd, enc, &answer);
                    let (locations, truncated) = query::cap(all);
                    let mut out = serde_json::json!({
                        "status": "ok",
                        "locations": locations,
                        "truncated": truncated,
                    });
                    if truncated {
                        out["hint"] = "more references exist; ask about a more specific symbol, or search a \
                                       narrower folder with grep"
                            .into();
                    }
                    out
                }
                Err(status) => status,
            },
        )
    }
}
