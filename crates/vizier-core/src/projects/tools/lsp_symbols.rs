//! `lsp_symbols`: what a file declares, or a search for symbols by name.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::lsp_common::{existing_file, project};
use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    projects::lsp::{
        languages::language_for,
        position::{column_from_lsp, relative, uri_to_path},
        query,
    },
};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct LspSymbolsInput {
    /// A file to list the symbols of, or with `query` a file or folder that picks the root to search.
    pub path: String,
    /// Search for symbols whose name matches this, in the root `path` is in.
    #[serde(default)]
    pub query: Option<String>,
}

pub struct LspSymbols;

const KINDS: [&str; 26] = [
    "file", "module", "namespace", "package", "class", "method", "property", "field",
    "constructor", "enum", "interface", "function", "variable", "constant", "string", "number",
    "boolean", "array", "object", "key", "null", "enum_member", "struct", "event", "operator",
    "type_parameter",
];

pub fn kind_name(kind: u64) -> &'static str {
    kind.checked_sub(1)
        .and_then(|n| KINDS.get(n as usize))
        .copied()
        .unwrap_or("unknown")
}

#[derive(Debug, Serialize)]
struct Symbol {
    name: String,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    container: Option<String>,
    path: String,
    line: usize,
}

/// `DocumentSymbol[]` (nested) or `SymbolInformation[]` (flat) to one flat list.
fn flatten(cwd: &Path, file_rel: &str, items: &[Value], parent: Option<&str>, out: &mut Vec<Symbol>) {
    for item in items {
        let name = item.get("name").and_then(Value::as_str).unwrap_or("").to_owned();
        let kind = kind_name(item.get("kind").and_then(Value::as_u64).unwrap_or(0));
        if let Some(location) = item.get("location") {
            // SymbolInformation: its own file.
            let uri = location.get("uri").and_then(Value::as_str).unwrap_or("");
            let line = location.pointer("/range/start/line").and_then(Value::as_u64).unwrap_or(0) as usize;
            if let Some(rel) = uri_to_path(uri).and_then(|p| relative(cwd, &p)) {
                if rel == ".vizier" || rel.starts_with(".vizier/") {
                    continue;
                }
                out.push(Symbol {
                    name,
                    kind,
                    container: item.get("containerName").and_then(Value::as_str).filter(|c| !c.is_empty()).map(str::to_owned),
                    path: rel,
                    line: line + 1,
                });
            }
        } else {
            let line = item
                .pointer("/selectionRange/start/line")
                .or_else(|| item.pointer("/range/start/line"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            out.push(Symbol {
                name: name.clone(),
                kind,
                container: parent.map(str::to_owned),
                path: file_rel.to_owned(),
                line: line + 1,
            });
            if let Some(children) = item.get("children").and_then(Value::as_array) {
                flatten(cwd, file_rel, children, Some(&name), out);
            }
        }
    }
}

/// The first source file under `folder` that a language owns, to learn which server a search asks.
fn first_source_file(folder: &Path) -> Option<PathBuf> {
    let mut queue = std::collections::VecDeque::from([folder.to_path_buf()]);
    let mut seen = 0usize;
    while let Some(dir) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut subdirs = vec![];
        for entry in entries.flatten() {
            seen += 1;
            if seen > 5_000 {
                return None;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if !matches!(name.as_str(), ".git" | ".vizier" | "node_modules" | "target" | "dist" | "build") {
                    subdirs.push(path);
                }
            } else if language_for(&path).is_some() {
                return Some(path);
            }
        }
        subdirs.sort();
        queue.extend(subdirs);
    }
    None
}

#[async_trait::async_trait]
impl VizierTool for LspSymbols {
    type Input = LspSymbolsInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "lsp_symbols".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn description(&self) -> String {
        "List what a file declares, with `path`; or search symbols by name across a root, with \
         `query` and a `path` inside the root to search (a repository can hold several roots and \
         a search reaches only one). Results are cut at 50."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let project = project(ctx)?;
        let query_text = args.query.as_deref().map(str::trim).filter(|q| !q.is_empty());
        let cwd = &project.cwd;

        let resolved = crate::projects::path::resolve(cwd, &args.path, crate::projects::path::Purpose::Existing)?;
        let file = if resolved.is_dir() {
            if query_text.is_none() {
                return Err(VizierError(format!("not_a_file: {} is a folder; name a file, or pass `query`", args.path)));
            }
            match first_source_file(&resolved) {
                Some(file) => file,
                None => return Ok(query::status("no_language")),
            }
        } else {
            existing_file(ctx, &args.path)?
        };

        let ready = match query::prepare(&ctx.session.0, project, &file).await {
            Ok(ready) => ready,
            Err(status) => return Ok(status),
        };
        let mut symbols: Vec<Symbol> = Vec::new();
        match query_text {
            Some(text) => {
                let answer = match query::ask(&ready.server, "workspace/symbol", json!({ "query": text })).await {
                    Ok(answer) => answer,
                    Err(status) => return Ok(status),
                };
                let list = answer.as_array().cloned().unwrap_or_default();
                flatten(cwd, "", &list, None, &mut symbols);
            }
            None => {
                let answer = match query::ask(
                    &ready.server,
                    "textDocument/documentSymbol",
                    json!({ "textDocument": { "uri": ready.uri } }),
                )
                .await
                {
                    Ok(answer) => answer,
                    Err(status) => return Ok(status),
                };
                let list = answer.as_array().cloned().unwrap_or_default();
                let rel = relative(cwd, &file).unwrap_or_default();
                flatten(cwd, &rel, &list, None, &mut symbols);
            }
        }
        let _ = column_from_lsp;
        let (symbols, truncated) = query::cap(symbols);
        let mut out = json!({ "status": "ok", "symbols": symbols, "truncated": truncated });
        if truncated {
            out["hint"] = "more symbols match; use a longer `query` or a path in a narrower root".into();
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_have_names() {
        assert_eq!(kind_name(12), "function");
        assert_eq!(kind_name(5), "class");
        assert_eq!(kind_name(0), "unknown");
        assert_eq!(kind_name(99), "unknown");
    }

    #[test]
    fn nested_document_symbols_are_flattened_with_their_container() {
        let items = json!([{
            "name": "Paths", "kind": 23,
            "selectionRange": {"start": {"line": 3, "character": 0}},
            "children": [{"name": "resolve", "kind": 6, "selectionRange": {"start": {"line": 5, "character": 4}}}]
        }]);
        let mut out = vec![];
        flatten(Path::new("/p"), "src/lib.rs", items.as_array().unwrap(), None, &mut out);
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].name, "resolve");
        assert_eq!(out[1].container.as_deref(), Some("Paths"));
        assert_eq!(out[1].line, 6);
        assert_eq!(out[1].path, "src/lib.rs");
    }
}
