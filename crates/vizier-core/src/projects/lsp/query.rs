//! Questions to a server (`contracts/lsp-tools.md`): resolve a file to its server, refuse in a
//! way a script can branch on while it is not ready, and turn answers into compact structured
//! data with no absolute host paths.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use super::position::{self, Encoding, Location};
use super::server::{RequestError, Server};
use super::sync::SyncError;
use super::{Availability, target_for};
use crate::projects::context::ProjectTurnContext;

/// Most items in a result, and the most bytes (FR-023).
pub const MAX_ITEMS: usize = 50;
pub const MAX_BYTES: usize = 16 * 1024;

/// A file whose server answered and now holds the file's current content.
pub struct Ready {
    pub server: Arc<Server>,
    pub file: PathBuf,
    pub bytes: Vec<u8>,
    pub text: String,
    pub uri: String,
    pub language_id: &'static str,
}

/// The `{status: …}` output a script reads when a question cannot be answered.
pub fn status(name: &str) -> Value {
    json!({ "status": name })
}

/// Find the server for `file` and make it hold the file. `Err` is a finished tool output.
pub async fn prepare(
    agent_id: &str,
    project: &ProjectTurnContext,
    file: &Path,
) -> Result<Ready, Value> {
    if !project.lsp_enabled {
        return Err(status("lsp_disabled"));
    }
    let Some(target) = target_for(agent_id, project, file) else {
        return Err(match crate::projects::lsp::languages::language_for(file) {
            Some(_) => status("lsp_disabled"),
            None => status("no_language"),
        });
    };
    let server = match project.lsp.ensure(&target) {
        Availability::Ready(server) => server,
        Availability::Closed => return Err(status("lsp_disabled")),
        Availability::Busy => return Err(status("lsp_busy")),
        Availability::Starting { detail } => {
            return Err(json!({ "status": "lsp_starting", "detail": detail }));
        }
        Availability::Missing { command, install_hint, .. } => {
            return Err(json!({
                "status": "lsp_unavailable",
                "server": command,
                "install_hint": install_hint,
            }));
        }
        Availability::Failed { reason } => {
            return Err(json!({ "status": "lsp_failed", "reason": first_line(&reason) }));
        }
    };
    let bytes = tokio::fs::read(file)
        .await
        .map_err(|err| json!({ "status": "file_unsupported", "reason": err.to_string() }))?;
    let language_id = target.language.lsp_language_id(file);
    let synced = server
        .sync_document(file, &bytes, language_id)
        .await
        .map_err(|SyncError::Unsupported| status("file_unsupported"))?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok(Ready {
        server,
        file: file.to_path_buf(),
        bytes,
        text,
        uri: synced.uri,
        language_id,
    })
}

pub fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or("").trim().chars().take(300).collect()
}

/// Send a request and turn a failure into a status output.
pub async fn ask(server: &Server, method: &str, params: Value) -> Result<Value, Value> {
    match server.request(method, params).await {
        Ok(v) => Ok(v),
        Err(RequestError::Timeout) => Err(status("lsp_timeout")),
        Err(RequestError::Exited(reason)) => {
            Err(json!({ "status": "lsp_failed", "reason": first_line(&reason) }))
        }
        Err(RequestError::Server(e)) if e.code == -32801 || e.code == -32802 => {
            Err(json!({ "status": "lsp_starting", "detail": "still working on it" }))
        }
        Err(RequestError::Server(e)) => Err(json!({
            "status": "lsp_failed",
            "reason": format!("the server could not answer: {}", first_line(&e.message)),
        })),
    }
}

/// Where a symbol sits in a file: the 0-based line and the server's offset on it.
pub fn locate(
    text: &str,
    line: usize,
    symbol: &str,
    occurrence: Option<usize>,
    enc: Encoding,
) -> Result<(u32, u32), Value> {
    let line_text = line
        .checked_sub(1)
        .and_then(|n| text.lines().nth(n))
        .unwrap_or("");
    match position::locate_symbol(line_text, symbol, occurrence) {
        Some(column) => Ok(((line - 1) as u32, position::column_to_lsp(line_text, column, enc))),
        None => Err(json!({
            "status": "symbol_not_on_line",
            "line_text": position::preview(line_text),
        })),
    }
}

/// `Location`, `Location[]`, `LocationLink[]` or null to mapped locations. Locations outside the
/// project are marked as such; those under `.vizier/` are dropped.
pub fn map_locations(cwd: &Path, enc: Encoding, answer: &Value) -> Vec<Location> {
    let items: Vec<&Value> = match answer {
        Value::Array(list) => list.iter().collect(),
        Value::Null => vec![],
        single => vec![single],
    };
    let mut out = Vec::new();
    for item in items {
        let uri = item
            .get("uri")
            .or_else(|| item.get("targetUri"))
            .and_then(Value::as_str);
        let start = item
            .pointer("/range/start")
            .or_else(|| item.pointer("/targetSelectionRange/start"));
        let (Some(uri), Some(start)) = (uri, start) else { continue };
        let line0 = start.get("line").and_then(Value::as_u64).unwrap_or(0) as u32;
        let offset = start.get("character").and_then(Value::as_u64).unwrap_or(0) as u32;
        let read = |path: &Path, line: u32| -> Option<String> {
            let text = std::fs::read_to_string(path).ok()?;
            text.lines().nth(line as usize).map(str::to_owned)
        };
        if let Some(location) = position::map_location(cwd, uri, line0, offset, enc, &read) {
            out.push(location);
        }
    }
    out
}

/// Cut a list to [`MAX_ITEMS`] and [`MAX_BYTES`], saying so and how to narrow it.
pub fn cap<T: serde::Serialize>(items: Vec<T>) -> (Vec<T>, bool) {
    let total = items.len();
    let mut kept = Vec::new();
    let mut bytes = 0usize;
    for item in items {
        let size = serde_json::to_string(&item).map(|s| s.len()).unwrap_or(0);
        if kept.len() >= MAX_ITEMS || bytes + size > MAX_BYTES {
            break;
        }
        bytes += size;
        kept.push(item);
    }
    let truncated = kept.len() < total;
    (kept, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locate_finds_the_symbol_and_converts_the_column() {
        // "日本 name": `name` is the 4th scalar, utf-16 offset 3.
        let got = locate("x\n日本 name\n", 2, "name", None, Encoding::Utf16).unwrap();
        assert_eq!(got, (1, 3));
        let got8 = locate("x\n日本 name\n", 2, "name", None, Encoding::Utf8).unwrap();
        assert_eq!(got8, (1, 7));
    }

    #[test]
    fn a_missing_symbol_returns_the_line_text() {
        let err = locate("fn a() {}\n", 1, "b", None, Encoding::Utf16).unwrap_err();
        assert_eq!(err["status"], "symbol_not_on_line");
        assert_eq!(err["line_text"], "fn a() {}");
        let past = locate("fn a() {}\n", 9, "a", None, Encoding::Utf16).unwrap_err();
        assert_eq!(past["status"], "symbol_not_on_line");
    }

    #[test]
    fn the_cap_stops_at_fifty() {
        let (kept, truncated) = cap((0..80).collect::<Vec<u32>>());
        assert_eq!(kept.len(), MAX_ITEMS);
        assert!(truncated);
        let (kept, truncated) = cap(vec![1, 2, 3]);
        assert_eq!(kept.len(), 3);
        assert!(!truncated);
    }
}
