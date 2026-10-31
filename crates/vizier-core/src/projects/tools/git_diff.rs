//! `git_diff`: unified diff against `HEAD` (or the index), bounded.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    projects::git::{DIFF_CAP_BYTES, Git},
};

use super::cwd;

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct GitDiffInput {
    /// One file relative to the project folder; the whole working tree when omitted.
    #[serde(default)]
    pub path: Option<String>,
    /// Diff the index instead of the working tree.
    #[serde(default)]
    pub staged: Option<bool>,
}

pub struct GitDiff;

#[async_trait::async_trait]
impl VizierTool for GitDiff {
    type Input = GitDiffInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "git_diff".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn description(&self) -> String {
        "Show the unified diff of the project folder's uncommitted changes against the last \
         commit, whole tree or one file, capped at 256 KiB."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let git = Git::new(&cwd(ctx)?);
        match args.path {
            Some(path) => {
                let diff = git.file_diff(&path, args.staged.unwrap_or(false)).await?;
                Ok(serde_json::to_value(diff)
                    .map_err(|err| VizierError(err.to_string()))?)
            }
            None => {
                let args = if args.staged.unwrap_or(false) {
                    vec!["diff", "--cached"]
                } else {
                    vec!["diff", "HEAD"]
                };
                let mut diff = git.run_read(&args).await?;
                let truncated = diff.len() > DIFF_CAP_BYTES;
                if truncated {
                    diff.truncate(DIFF_CAP_BYTES);
                }
                Ok(serde_json::json!({ "diff": diff, "truncated": truncated, "binary": false }))
            }
        }
    }
}
