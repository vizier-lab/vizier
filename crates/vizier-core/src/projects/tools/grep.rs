//! `grep`: regex search over the project's text files.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    projects::path,
};

use super::{cwd, walk};

/// Lines longer than this are cut before being returned.
pub const LINE_CAP_CHARS: usize = 500;
/// A file bigger than this is skipped and counted.
pub const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
/// The default and maximum result counts.
pub const DEFAULT_RESULTS: usize = 200;
pub const MAX_RESULTS: usize = 1_000;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct GrepInput {
    /// A regular expression.
    pub pattern: String,
    /// A folder or file relative to the project folder; the root when omitted.
    #[serde(default)]
    pub path: Option<String>,
    /// Only search files whose path matches this gitignore-style glob.
    #[serde(default)]
    pub glob: Option<String>,
    #[serde(default)]
    pub case_insensitive: Option<bool>,
    /// Context lines before and after each match, 0–5 (default 0).
    #[serde(default)]
    pub context: Option<usize>,
    /// Most matches to return, 1–1,000 (default 200).
    #[serde(default)]
    pub max_results: Option<usize>,
}

pub struct Grep;

fn search(
    cwd: &std::path::Path,
    root: &std::path::Path,
    args: &GrepInput,
) -> Result<serde_json::Value, VizierError> {
    let mut builder = regex::RegexBuilder::new(&args.pattern);
    builder.case_insensitive(args.case_insensitive.unwrap_or(false));
    let regex = builder
        .build()
        .map_err(|err| VizierError(format!("invalid_pattern: {err}")))?;
    let context = args.context.unwrap_or(0).min(5);
    let max_results = args.max_results.unwrap_or(DEFAULT_RESULTS).clamp(1, MAX_RESULTS);
    let glob = args
        .glob
        .as_deref()
        .map(|pattern| {
            let mut overrides = ignore::overrides::OverrideBuilder::new(root);
            overrides
                .add(pattern)
                .map_err(|err| VizierError(format!("invalid_pattern: {err}")))?;
            overrides
                .build()
                .map_err(|err| VizierError(format!("invalid_pattern: {err}")))
        })
        .transpose()?;

    let mut files = vec![];
    if root.is_file() {
        files.push(root.to_path_buf());
    } else {
        let mut walker = walk::builder(root, false);
        if let Some(overrides) = glob {
            walker.overrides(overrides);
        }
        for entry in walker.build().flatten() {
            if entry.depth() == 0 {
                continue;
            }
            if entry.file_type().is_some_and(|kind| kind.is_file()) {
                files.push(entry.into_path());
            }
        }
    }

    let mut matches = vec![];
    let mut files_searched = 0u64;
    let mut files_skipped = 0u64;
    let mut truncated = false;
    'files: for file in files {
        let Ok(meta) = std::fs::metadata(&file) else {
            files_skipped += 1;
            continue;
        };
        if meta.len() > MAX_FILE_BYTES {
            files_skipped += 1;
            continue;
        }
        let Ok(bytes) = std::fs::read(&file) else {
            files_skipped += 1;
            continue;
        };
        if walk::is_binary(&bytes) {
            files_skipped += 1;
            continue;
        }
        files_searched += 1;
        let shown = path::display_rel(cwd, &file);
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<&str> = text.split('\n').collect();
        for (index, line) in lines.iter().enumerate() {
            if !regex.is_match(line) {
                continue;
            }
            if matches.len() >= max_results {
                truncated = true;
                break 'files;
            }
            let cut = |line: &str| -> String {
                if line.chars().count() > LINE_CAP_CHARS {
                    let mut out: String = line.chars().take(LINE_CAP_CHARS).collect();
                    out.push('…');
                    out
                } else {
                    line.trim_end_matches('\r').to_string()
                }
            };
            let before: Vec<String> = (index.saturating_sub(context)..index)
                .map(|i| cut(lines[i]))
                .collect();
            let after: Vec<String> = (index + 1..(index + 1 + context).min(lines.len()))
                .filter(|i| !(lines[*i].is_empty() && *i == lines.len() - 1))
                .map(|i| cut(lines[i]))
                .collect();
            matches.push(serde_json::json!({
                "path": shown,
                "line": index + 1,
                "text": cut(line),
                "before": before,
                "after": after,
            }));
        }
    }

    Ok(serde_json::json!({
        "matches": matches,
        "files_searched": files_searched,
        "files_skipped": files_skipped,
        "truncated": truncated,
    }))
}

#[async_trait::async_trait]
impl VizierTool for Grep {
    type Input = GrepInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "grep".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn description(&self) -> String {
        "Search the project's text files with a regular expression, optionally only files matching \
         a glob and with 0–5 context lines. Binary files and files over 4 MiB are skipped and \
         counted. Lines are cut at 500 characters; results are bounded."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let cwd = cwd(ctx)?;
        let root = path::resolve(
            &cwd,
            args.path.as_deref().unwrap_or(""),
            path::Purpose::Existing,
        )?;
        let cwd_for = cwd.clone();
        let args_for = args.clone();
        let base = root.clone();
        let value = tokio::task::spawn_blocking(move || search(&cwd_for, &base, &args_for))
            .await
            .map_err(|err| VizierError(err.to_string()))??;
        // `grep` is a search, not a read: it does not let a later edit skip `read_file`
        // (quickstart §3).
        Ok(value)
    }
}
