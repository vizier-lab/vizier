//! What the five `lsp_*` tools share (`specs/054-project-lsp/contracts/lsp-tools.md`): the
//! position input, resolving it to a server and a question, and the status outputs.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    agents::tools::ToolContext,
    error::VizierError,
    projects::{
        context::ProjectTurnContext,
        lsp::{
            position::{Encoding, lsp_position},
            query::{self, Ready},
        },
        path,
    },
};

use super::cwd;

/// Where a symbol is: a file, a 1-based line and the symbol's name, never a column.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct PositionInput {
    /// A file path relative to the project folder.
    pub path: String,
    /// The 1-based line the symbol is on.
    pub line: usize,
    /// The symbol's name as written on that line.
    pub symbol: String,
    /// Which match on the line when the name occurs more than once, starting at 1.
    #[serde(default)]
    pub occurrence: Option<usize>,
}

pub(super) fn project(ctx: &ToolContext) -> Result<&ProjectTurnContext, VizierError> {
    ctx.project
        .as_deref()
        .ok_or_else(|| VizierError("project_unavailable: this conversation is not a project session".into()))
}

/// The file a tool names, inside the project, which must exist and be a file.
pub(super) fn existing_file(ctx: &ToolContext, requested: &str) -> Result<PathBuf, VizierError> {
    let cwd = cwd(ctx)?;
    let file = path::resolve(&cwd, requested, path::Purpose::Existing)?;
    if !file.is_file() {
        return Err(VizierError(format!("not_a_file: {requested}")));
    }
    Ok(file)
}

/// Resolve a position question up to the answer: the server, the file synced, the symbol located
/// and the request sent. `Err` is a finished output (`lsp_starting`, `symbol_not_on_line`, …).
pub(super) async fn position_request(
    ctx: &ToolContext,
    input: &PositionInput,
    method: &str,
    extra: Option<Value>,
) -> Result<(Ready, Value, Encoding), Value> {
    let project = project(ctx).map_err(|e| serde_json::json!({ "status": "lsp_failed", "reason": e.0 }))?;
    let file = existing_file(ctx, &input.path).map_err(|e| serde_json::json!({ "status": "not_a_file", "reason": e.0 }))?;
    let ready = query::prepare(&ctx.session.0, project, &file).await?;
    let enc = ready.server.encoding();
    let (line0, character) = query::locate(&ready.text, input.line, &input.symbol, input.occurrence, enc)?;
    let mut params = serde_json::json!({
        "textDocument": { "uri": ready.uri },
        "position": lsp_position(line0, character),
    });
    if let (Some(Value::Object(more)), Some(map)) = (extra, params.as_object_mut()) {
        map.extend(more);
    }
    let answer = query::ask(&ready.server, method, params).await?;
    Ok((ready, answer, enc))
}
