//! The wait after an edit and the answer it produces (R6, `contracts/lsp-tools.md`).
//!
//! The rule that matters: `current` is claimed only when an answer reflects *this* edit and the
//! server reports no work in progress. Anything else is `pending`, which never means "clean".

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Value, json};

use super::position::{Encoding, column_from_lsp, relative, uri_to_path};
use super::server::{RequestError, Server};
use super::sync::{Synced, SyncError};

/// How long a write waits for the answer (FR-028).
pub const WAIT: Duration = Duration::from_secs(3);
/// Progress that opens within this long of the save still counts as "running" (R6).
const PROGRESS_GRACE: Duration = Duration::from_millis(300);
/// Other open documents asked about after a change, at most.
const OTHER_FILES: usize = 20;
pub const MAX_ITEMS: usize = 50;
pub const MAX_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Item {
    pub path: String,
    pub line: usize,
    pub character: usize,
    pub severity: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OtherFile {
    pub path: String,
    pub errors: usize,
    pub items: Vec<Item>,
}

/// What an edit's result carries as `diagnostics`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiagnosticsResult {
    /// `current`, `pending` or `unavailable`.
    pub status: &'static str,
    pub errors: usize,
    pub warnings: usize,
    pub items: Vec<Item>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub other_files: Vec<OtherFile>,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl DiagnosticsResult {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: "unavailable",
            errors: 0,
            warnings: 0,
            items: vec![],
            other_files: vec![],
            truncated: false,
            message: Some(message.into()),
        }
    }

    pub fn pending() -> Self {
        Self {
            status: "pending",
            errors: 0,
            warnings: 0,
            items: vec![],
            other_files: vec![],
            truncated: false,
            message: None,
        }
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

fn severity_of(raw: &Value) -> Option<&'static str> {
    match raw.get("severity").and_then(Value::as_u64) {
        Some(1) | None => Some("error"),
        Some(2) => Some("warning"),
        // Information and hints are left out (contract).
        _ => None,
    }
}

fn read_line(path: &Path, line0: usize) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().nth(line0).map(str::to_owned)
}

/// Turn one document's raw diagnostics into items. `None` for a file outside `cwd` or under
/// `.vizier/`, which is never reported (R11).
pub fn items_of(
    cwd: &Path,
    uri: &str,
    raw: &[Value],
    enc: Encoding,
    text_of: &dyn Fn(&Path) -> Option<String>,
) -> Option<Vec<Item>> {
    let path = uri_to_path(uri)?;
    let rel = relative(cwd, &path)?;
    if rel == ".vizier" || rel.starts_with(".vizier/") {
        return None;
    }
    let text = text_of(&path);
    let mut items: Vec<Item> = raw
        .iter()
        .filter_map(|d| {
            let severity = severity_of(d)?;
            let start = d.pointer("/range/start")?;
            let line0 = start.get("line")?.as_u64()? as usize;
            let offset = start.get("character").and_then(Value::as_u64).unwrap_or(0) as u32;
            let line_text = text
                .as_deref()
                .and_then(|t| t.lines().nth(line0).map(str::to_owned))
                .or_else(|| read_line(&path, line0))
                .unwrap_or_default();
            Some(Item {
                path: rel.clone(),
                line: line0 + 1,
                character: column_from_lsp(&line_text, offset, enc),
                severity,
                message: d.get("message").and_then(Value::as_str).unwrap_or("").to_owned(),
                code: d.get("code").map(|c| match c {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                }),
                source: d.get("source").and_then(Value::as_str).map(str::to_owned),
            })
        })
        .collect();
    items.sort_by(|a, b| {
        (a.severity != "error", a.line, a.character).cmp(&(b.severity != "error", b.line, b.character))
    });
    Some(items)
}

/// Cut a list to the item and byte limits, reporting whether anything was dropped.
pub fn cap(items: Vec<Item>) -> (Vec<Item>, bool) {
    let mut kept = Vec::new();
    let mut bytes = 0usize;
    let total = items.len();
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

fn count(items: &[Item], severity: &str) -> usize {
    items.iter().filter(|i| i.severity == severity).count()
}

/// Ask a pull server about one document. `None` when it has nothing to say or does not answer.
async fn pull(server: &Server, uri: &str, limit: Duration) -> Option<Vec<Value>> {
    if limit.is_zero() {
        return None;
    }
    let call = server.request("textDocument/diagnostic", json!({ "textDocument": { "uri": uri } }));
    let answer = tokio::time::timeout(limit, call).await.ok()?;
    match answer {
        Ok(report) => Some(
            report
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        ),
        Err(RequestError::Server(_)) | Err(RequestError::Timeout) | Err(RequestError::Exited(_)) => None,
    }
}

/// Sync the file the way an edit leaves it, wait up to [`WAIT`], and report (R6).
pub async fn wait_after_write(
    server: &Arc<Server>,
    cwd: &Path,
    path: &Path,
    bytes: &[u8],
    language_id: &str,
) -> DiagnosticsResult {
    let started = Instant::now();
    let seq_before = *server.push_seq.lock();
    let synced = match server.sync_document(path, bytes, language_id).await {
        Ok(synced) => synced,
        Err(SyncError::Unsupported) => {
            return DiagnosticsResult::unavailable(
                "this file is too large, or not UTF-8, for the language server",
            );
        }
    };
    server.did_save(&synced.uri);
    let save_time = Instant::now();
    let enc = server.encoding();
    let caps = server.capabilities();
    let text = String::from_utf8_lossy(bytes).into_owned();
    let own_path = path.to_path_buf();
    let text_of = move |p: &Path| (p == own_path).then(|| text.clone());

    let remaining = || WAIT.saturating_sub(started.elapsed());
    let mut own: Option<Vec<Value>> = None;
    let mut others: Vec<(String, Vec<Value>)> = Vec::new();

    if caps.pull_diagnostics {
        own = pull(server, &synced.uri, remaining()).await;
        if own.is_some() && caps.inter_file_dependencies {
            let uris: Vec<String> = server
                .open_documents()
                .await
                .into_iter()
                .filter(|u| *u != synced.uri)
                .take(OTHER_FILES)
                .collect();
            let asks = uris.iter().map(|u| async {
                let r = pull(server, u, remaining()).await;
                (u.clone(), r)
            });
            for (uri, raw) in futures::future::join_all(asks).await {
                if let Some(raw) = raw {
                    others.push((uri, raw));
                }
            }
        }
    } else {
        own = wait_for_push(server, &synced, seq_before, remaining()).await;
        let pushed = server.pushed.lock();
        for (uri, entry) in pushed.iter() {
            if *uri != synced.uri && entry.seq > seq_before {
                others.push((uri.clone(), entry.items.clone()));
            }
        }
    }

    // Wait out the grace period before judging whether work is running.
    let grace_end = save_time + PROGRESS_GRACE;
    if Instant::now() < grace_end {
        tokio::time::sleep_until(grace_end.into()).await;
    }
    let working = server.is_busy(Duration::ZERO);

    let Some(own) = own else {
        // No answer that reflects this edit: never show what came before.
        return DiagnosticsResult::pending();
    };
    let items = items_of(cwd, &synced.uri, &own, enc, &text_of).unwrap_or_default();
    let errors = count(&items, "error");
    let warnings = count(&items, "warning");
    let (items, mut truncated) = cap(items);

    let mut other_files = Vec::new();
    for (uri, raw) in others {
        let Some(list) = items_of(cwd, &uri, &raw, enc, &|_| None) else {
            continue;
        };
        let errs = count(&list, "error");
        if errs == 0 {
            continue;
        }
        let (list, cut) = cap(list.into_iter().filter(|i| i.severity == "error").collect());
        truncated |= cut;
        if let Some(first) = list.first() {
            other_files.push(OtherFile { path: first.path.clone(), errors: errs, items: list });
        }
    }
    other_files.sort_by(|a, b| a.path.cmp(&b.path));
    other_files.truncate(OTHER_FILES);

    DiagnosticsResult {
        status: if working { "pending" } else { "current" },
        errors,
        warnings,
        items,
        other_files,
        truncated,
        message: None,
    }
}

/// What a server has published (or answers about its open documents) for files under `folder`,
/// for `lsp_diagnostics` on a folder. Errors first; callers cap.
pub async fn reported(server: &Arc<Server>, cwd: &Path, folder: &Path) -> Vec<Item> {
    let enc = server.encoding();
    let mut items: Vec<Item> = Vec::new();
    let under = |uri: &str| uri_to_path(uri).is_some_and(|p| p.starts_with(folder));
    if server.capabilities().pull_diagnostics {
        let uris: Vec<String> = server
            .open_documents()
            .await
            .into_iter()
            .filter(|u| under(u))
            .take(OTHER_FILES)
            .collect();
        let asks = uris.iter().map(|u| async { (u.clone(), pull(server, u, WAIT).await) });
        for (uri, raw) in futures::future::join_all(asks).await {
            if let Some(list) = raw.and_then(|raw| items_of(cwd, &uri, &raw, enc, &|_| None)) {
                items.extend(list);
            }
        }
    } else {
        let pushed: Vec<(String, Vec<Value>)> = server
            .pushed
            .lock()
            .iter()
            .filter(|(uri, _)| under(uri))
            .map(|(uri, entry)| (uri.clone(), entry.items.clone()))
            .collect();
        for (uri, raw) in pushed {
            if let Some(list) = items_of(cwd, &uri, &raw, enc, &|_| None) {
                items.extend(list);
            }
        }
    }
    items.sort_by(|a, b| {
        (a.severity != "error", &a.path, a.line).cmp(&(b.severity != "error", &b.path, b.line))
    });
    items
}

/// A push that reflects the synced version: versioned and not older, or unversioned and received
/// after the sync.
async fn wait_for_push(
    server: &Server,
    synced: &Synced,
    seq_before: u64,
    limit: Duration,
) -> Option<Vec<Value>> {
    let deadline = Instant::now() + limit;
    loop {
        let notified = server.activity.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if let Some(entry) = server.pushed.lock().get(&synced.uri) {
            let fresh = match entry.version {
                Some(v) => v >= synced.version,
                None => entry.seq > seq_before,
            };
            if fresh {
                return Some(entry.items.clone());
            }
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || tokio::time::timeout(left, notified).await.is_err() {
            return None;
        }
    }
}

/// Where a path is for the agent; used by tools that report a file.
pub fn display_path(cwd: &Path, path: &Path) -> Option<String> {
    relative(cwd, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(line: usize, col: usize, severity: u64, msg: &str) -> Value {
        json!({"range":{"start":{"line":line,"character":col},"end":{"line":line,"character":col+1}},
               "severity":severity,"message":msg,"code":"E1","source":"fake"})
    }

    fn uri_in(cwd: &Path, rel: &str) -> String {
        super::super::position::path_to_uri(&cwd.join(rel)).unwrap()
    }

    #[test]
    fn errors_sort_before_warnings_and_hints_are_dropped() {
        let cwd = std::env::temp_dir().join("diag-proj");
        let uri = uri_in(&cwd, "a.rs");
        let list = vec![raw(0, 0, 2, "w"), raw(5, 2, 1, "e"), raw(1, 0, 4, "hint"), raw(2, 0, 3, "info")];
        let items = items_of(&cwd, &uri, &list, Encoding::Utf16, &|_| Some("x\n".into())).unwrap();
        assert_eq!(items.iter().map(|i| i.message.as_str()).collect::<Vec<_>>(), vec!["e", "w"]);
        assert_eq!(items[0].line, 6);
        assert_eq!(items[0].path, "a.rs");
    }

    #[test]
    fn a_file_outside_the_project_or_under_vizier_is_never_reported() {
        let cwd = std::env::temp_dir().join("diag-proj");
        let outside = super::super::position::path_to_uri(&std::env::temp_dir().join("elsewhere.rs")).unwrap();
        assert!(items_of(&cwd, &outside, &[raw(0, 0, 1, "e")], Encoding::Utf16, &|_| None).is_none());
        let hidden = uri_in(&cwd, ".vizier/agent/wt/a.rs");
        assert!(items_of(&cwd, &hidden, &[raw(0, 0, 1, "e")], Encoding::Utf16, &|_| None).is_none());
    }

    #[test]
    fn the_cap_limits_items_and_says_so() {
        let cwd = std::env::temp_dir().join("diag-proj");
        let uri = uri_in(&cwd, "a.rs");
        let list: Vec<Value> = (0..80).map(|i| raw(i, 0, 1, "e")).collect();
        let items = items_of(&cwd, &uri, &list, Encoding::Utf16, &|_| Some("x".into())).unwrap();
        let (kept, truncated) = cap(items);
        assert_eq!(kept.len(), MAX_ITEMS);
        assert!(truncated);
    }

    #[test]
    fn the_column_follows_the_servers_encoding() {
        let cwd = std::env::temp_dir().join("diag-proj");
        let uri = uri_in(&cwd, "a.rs");
        // "a😀b": the error is on `b`, utf-16 offset 3.
        let items = items_of(&cwd, &uri, &[raw(0, 3, 1, "e")], Encoding::Utf16, &|_| Some("a😀b".into())).unwrap();
        assert_eq!(items[0].character, 3);
    }
}
