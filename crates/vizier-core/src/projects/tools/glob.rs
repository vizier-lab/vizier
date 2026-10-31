//! `glob`: find files by a gitignore-style pattern.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    projects::path,
};

use super::{cwd, walk};

/// The most paths one glob returns.
pub const MAX_PATHS: usize = 1_000;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct GlobInput {
    /// A gitignore-style pattern, for example `**/*.rs`.
    pub pattern: String,
    /// A folder relative to the project folder to search under; the root when omitted.
    #[serde(default)]
    pub path: Option<String>,
}

pub struct Glob;

fn matches(cwd: &std::path::Path, root: &std::path::Path, pattern: &str) -> Result<Vec<String>, VizierError> {
    let mut overrides = ignore::overrides::OverrideBuilder::new(root);
    overrides
        .add(pattern)
        .map_err(|err| VizierError(format!("invalid_pattern: {err}")))?;
    let overrides = overrides
        .build()
        .map_err(|err| VizierError(format!("invalid_pattern: {err}")))?;
    let mut builder = walk::builder(root, false);
    builder.overrides(overrides);
    let mut paths: Vec<String> = builder
        .build()
        .flatten()
        .filter(|entry| entry.depth() > 0)
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .map(|entry| path::display_rel(cwd, entry.path()))
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

#[async_trait::async_trait]
impl VizierTool for Glob {
    type Input = GlobInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "glob".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn description(&self) -> String {
        "Find files in the project whose path matches a gitignore-style glob such as `**/*.rs`. \
         Files only, sorted, at most 1,000, with the project's ignore rules applied."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let cwd = cwd(ctx)?;
        let root = path::resolve(
            &cwd,
            args.path.as_deref().unwrap_or(""),
            path::Purpose::Existing,
        )?;
        let pattern = args.pattern.clone();
        let base = root.clone();
        let cwd_for_paths = cwd.clone();
        let mut paths = tokio::task::spawn_blocking(move || matches(&cwd_for_paths, &base, &pattern))
            .await
            .map_err(|err| VizierError(err.to_string()))??;
        let truncated = paths.len() > MAX_PATHS;
        paths.truncate(MAX_PATHS);
        Ok(serde_json::json!({ "paths": paths, "truncated": truncated }))
    }
}
