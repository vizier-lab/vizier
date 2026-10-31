//! `read_file`: bounded, line-numbered reads inside the chat's folder.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    projects::path,
};

use super::cwd;

/// The most lines a read returns without a range.
pub const DEFAULT_LINES: usize = 2_000;
/// The most characters one line is returned with.
pub const LINE_CAP_CHARS: usize = 2_000;
/// The most bytes one read's `content` may hold.
pub const CONTENT_CAP_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ReadFileInput {
    /// A path relative to the project folder.
    pub path: String,
    /// First line, 1-based and inclusive.
    #[serde(default)]
    pub start_line: Option<usize>,
    /// Last line, 1-based and inclusive.
    #[serde(default)]
    pub end_line: Option<usize>,
}

pub struct ReadFile;

pub fn total_lines(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let newlines = text.bytes().filter(|b| *b == b'\n').count();
    newlines + usize::from(!text.ends_with('\n'))
}

fn line_at(text: &str, n: usize) -> &str {
    text.split('\n').nth(n.saturating_sub(1)).unwrap_or("")
}

#[async_trait::async_trait]
impl VizierTool for ReadFile {
    type Input = ReadFileInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "read_file".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn description(&self) -> String {
        "Read a file in the project folder, optionally a 1-based line range. Returns numbered \
         lines, the total line count and how to continue when the result is cut off (default \
         2,000 lines). A binary file is reported as binary with its size."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let cwd: PathBuf = cwd(ctx)?;
        let file = path::resolve(&cwd, &args.path, path::Purpose::Existing)?;
        if !file.is_file() {
            return Err(VizierError(format!("not_a_file: {}", args.path)));
        }
        let bytes = tokio::fs::read(&file)
            .await
            .map_err(|err| VizierError(format!("not_found: {} ({err})", args.path)))?;
        let shown = path::display_rel(&cwd, &file);
        if super::walk::is_binary(&bytes) {
            return Ok(serde_json::json!({
                "path": shown,
                "binary": true,
                "size": bytes.len(),
            }));
        }
        // The agent has now read the whole file, so a later edit may use any of it.
        super::record_read(ctx, &file, &bytes);

        let text = String::from_utf8_lossy(&bytes);
        let total = total_lines(&text);
        let start = args.start_line.unwrap_or(1).max(1);
        let end = match args.end_line {
            Some(end) => end.max(start),
            None => start.saturating_add(DEFAULT_LINES - 1),
        };
        let end = end.min(total.max(1));

        let mut content = String::new();
        if total == 0 {
            return Ok(serde_json::json!({
                "path": shown,
                "start_line": start,
                "end_line": start,
                "total_lines": 0,
                "content": "",
                "truncated": false,
                "next_start_line": null,
            }));
        }
        let mut rendered_end = start.saturating_sub(1);
        let mut cut_for_size = false;
        for n in start..=end {
            let line = line_at(&text, n);
            let line = if line.chars().count() > LINE_CAP_CHARS {
                let mut cut: String = line.chars().take(LINE_CAP_CHARS).collect();
                cut.push('…');
                cut
            } else {
                line.trim_end_matches('\r').to_string()
            };
            let row = format!("{n:>6}\t{line}\n");
            if content.len() + row.len() > CONTENT_CAP_BYTES {
                cut_for_size = true;
                break;
            }
            content.push_str(&row);
            rendered_end = n;
        }

        let truncated = cut_for_size || rendered_end < total;
        Ok(serde_json::json!({
            "path": shown,
            "start_line": start,
            "end_line": rendered_end,
            "total_lines": total,
            "content": content,
            "truncated": truncated,
            "next_start_line": truncated.then_some(rendered_end + 1),
        }))
    }
}
