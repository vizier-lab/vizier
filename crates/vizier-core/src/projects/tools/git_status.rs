//! `git_status`: the folder's branch and uncommitted changes, for the agent.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    projects::git::Git,
};

use super::cwd;

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct GitStatusInput {}

pub struct GitStatus;

#[async_trait::async_trait]
impl VizierTool for GitStatus {
    type Input = GitStatusInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "git_status".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn description(&self) -> String {
        "Show the project folder's current branch, ahead/behind counts and every uncommitted \
         change with its added/removed line counts."
            .into()
    }

    async fn call(&self, _args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let git = Git::new(&cwd(ctx)?);
        let status = git.status().await?;
        let changes = git.changes().await?;
        Ok(serde_json::json!({
            "branch": status.branch,
            "detached": status.detached,
            "ahead": status.ahead,
            "behind": status.behind,
            "changes": changes,
        }))
    }
}
