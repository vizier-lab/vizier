//! `git_log`: recent commits, optionally for one path.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    projects::git::Git,
};

use super::cwd;

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct GitLogInput {
    /// How many commits, 1–50 (default 10).
    #[serde(default)]
    pub n: Option<usize>,
    /// Only commits touching this path.
    #[serde(default)]
    pub path: Option<String>,
}

pub struct GitLog;

#[async_trait::async_trait]
impl VizierTool for GitLog {
    type Input = GitLogInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "git_log".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn description(&self) -> String {
        "Show the most recent commits in the project folder (hash, author, date, subject), \
         optionally only those touching one path. At most 50."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let git = Git::new(&cwd(ctx)?);
        let commits = git.log(args.n.unwrap_or(10), args.path.as_deref()).await?;
        Ok(serde_json::json!({ "commits": commits }))
    }
}
