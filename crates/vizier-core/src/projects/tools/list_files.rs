//! `list_files`: a bounded, sorted, gitignore-aware listing.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    projects::path,
};

use super::{cwd, walk};

/// The most entries one listing returns.
pub const MAX_ENTRIES: usize = 1_000;
/// The deepest a listing goes when its input does not say otherwise.
pub const DEFAULT_DEPTH: usize = 3;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ListFilesInput {
    /// A folder relative to the project folder; the folder itself when omitted.
    #[serde(default)]
    pub path: Option<String>,
    /// How many folder levels below `path` to list, 1–10 (default 3).
    #[serde(default)]
    pub depth: Option<usize>,
    /// Include files the project's ignore rules exclude (default false).
    #[serde(default)]
    pub include_ignored: Option<bool>,
}

pub struct ListFiles;

#[async_trait::async_trait]
impl VizierTool for ListFiles {
    type Input = ListFilesInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "list_files".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn description(&self) -> String {
        "List files and folders under a folder in the project (default the project root), sorted \
         by path, with sizes. Ignores `.git/`, the worktree area and the project's ignore rules \
         unless `include_ignored` is set. Bounded to 1,000 entries."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let cwd = cwd(ctx)?;
        let root = path::resolve(
            &cwd,
            args.path.as_deref().unwrap_or(""),
            path::Purpose::Existing,
        )?;
        if !root.is_dir() {
            return Err(VizierError(format!("not_a_directory: {}", args.path.unwrap_or_default())));
        }
        let depth = args.depth.unwrap_or(DEFAULT_DEPTH).clamp(1, 10);
        let include_ignored = args.include_ignored.unwrap_or(false);
        let base = root.clone();
        let walked = tokio::task::spawn_blocking(move || walk::walk(&base, include_ignored))
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        let mut entries: Vec<(String, serde_json::Value)> = walked
            .into_iter()
            .filter(|entry| entry.path.strip_prefix(&root).map(|rel| rel.components().count()).unwrap_or(0) <= depth)
            .map(|entry| {
                let rel = path::display_rel(&root, &entry.path);
                let rel = if entry.is_dir { format!("{rel}/") } else { rel };
                let value = match std::fs::metadata(&entry.path) {
                    Ok(meta) if entry.is_dir => serde_json::json!({ "path": rel, "kind": "dir" }),
                    Ok(meta) => serde_json::json!({ "path": rel, "kind": "file", "size": meta.len() }),
                    Err(_) => serde_json::json!({ "path": rel, "kind": if entry.is_dir { "dir" } else { "file" } }),
                };
                (rel, value)
            })
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let truncated = entries.len() > MAX_ENTRIES;
        entries.truncate(MAX_ENTRIES);
        Ok(serde_json::json!({
            "entries": entries.into_iter().map(|(_, value)| value).collect::<Vec<_>>(),
            "truncated": truncated,
        }))
    }
}
