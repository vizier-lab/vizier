//! `lsp_diagnostics`: the problems in a file, or what the running servers report for a folder.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::lsp_common::project;
use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    projects::{
        lsp::{diagnostics, query},
        path,
    },
};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct LspDiagnosticsInput {
    /// A file or folder relative to the project folder; the whole project when empty.
    #[serde(default)]
    pub path: Option<String>,
}

pub struct LspDiagnostics;

#[async_trait::async_trait]
impl VizierTool for LspDiagnostics {
    type Input = LspDiagnosticsInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "lsp_diagnostics".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn description(&self) -> String {
        "List the problems the language server sees. For a file it asks the server directly. For a \
         folder it reports what the running servers have published about files they analysed \
         (`scope: \"reported\"`), which is not every file. `freshness: \"pending\"` means work is \
         still running, so an empty list is not proof the code is clean. Errors come first; hints \
         are left out."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let project = project(ctx)?;
        let requested = args.path.as_deref().unwrap_or("").trim();
        let target = if requested.is_empty() {
            project.cwd.clone()
        } else {
            path::resolve(&project.cwd, requested, path::Purpose::Existing)?
        };

        if target.is_file() {
            let ready = match query::prepare(&ctx.session.0, project, &target).await {
                Ok(ready) => ready,
                Err(status) => return Ok(status),
            };
            let result = diagnostics::wait_after_write(
                &ready.server,
                &project.cwd,
                &target,
                &ready.bytes,
                ready.language_id,
            )
            .await;
            return Ok(json!({
                "status": "ok",
                "freshness": result.status,
                "errors": result.errors,
                "warnings": result.warnings,
                "items": result.items,
                "truncated": result.truncated,
            }));
        }

        // A folder: only what running servers already know.
        if !project.lsp_enabled {
            return Ok(query::status("lsp_disabled"));
        }
        let servers: Vec<_> = project
            .lsp
            .ready_servers(&project.project_id)
            .into_iter()
            .filter(|(root, _)| root.starts_with(&target) || target.starts_with(root))
            .collect();
        if servers.is_empty() {
            return Ok(json!({
                "status": "ok",
                "freshness": "pending",
                "scope": "reported",
                "errors": 0,
                "warnings": 0,
                "items": [],
                "truncated": false,
                "message": "no language server has analysed files here yet; read a file to start one",
            }));
        }
        let mut items = Vec::new();
        let mut working = false;
        for (_root, server) in &servers {
            working |= server.is_busy(std::time::Duration::from_millis(300));
            items.extend(diagnostics::reported(server, &project.cwd, &target).await);
        }
        items.sort_by(|a, b| (a.severity != "error", &a.path, a.line).cmp(&(b.severity != "error", &b.path, b.line)));
        items.dedup_by(|a, b| a == b);
        let errors = items.iter().filter(|i| i.severity == "error").count();
        let warnings = items.iter().filter(|i| i.severity == "warning").count();
        let (items, truncated) = diagnostics::cap(items);
        let mut out: Value = json!({
            "status": "ok",
            "freshness": if working { "pending" } else { "current" },
            "scope": "reported",
            "errors": errors,
            "warnings": warnings,
            "items": items,
            "truncated": truncated,
        });
        if truncated {
            out["hint"] = "more problems exist; ask about one folder or file at a time".into();
        }
        Ok(out)
    }
}
