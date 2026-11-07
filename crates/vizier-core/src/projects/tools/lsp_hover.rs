//! `lsp_hover`: a symbol's type signature and documentation.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::lsp_common::{PositionInput, position_request};
use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
};

/// Documentation is cut at this many bytes.
const DOCS_CAP: usize = 4 * 1024;

pub struct LspHover;

/// Hover contents are a string, a `{kind|language, value}` object, or a list of those.
fn text_of(contents: &Value) -> String {
    match contents {
        Value::String(s) => s.clone(),
        Value::Object(map) => match (map.get("language").and_then(Value::as_str), map.get("value").and_then(Value::as_str)) {
            (Some(language), Some(value)) => format!("```{language}\n{value}\n```"),
            (None, Some(value)) => value.to_owned(),
            _ => String::new(),
        },
        Value::Array(list) => list.iter().map(text_of).filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n\n"),
        _ => String::new(),
    }
}

/// Split hover markdown into the first fenced block (the signature) and everything else (the docs).
pub fn split_signature(markdown: &str) -> (Option<String>, String) {
    let Some(open) = markdown.find("```") else {
        let mut lines = markdown.lines();
        let first = lines.by_ref().find(|l| !l.trim().is_empty()).map(|l| l.trim().to_owned());
        return (first, lines.collect::<Vec<_>>().join("\n").trim().to_owned());
    };
    let after = &markdown[open + 3..];
    let body_start = after.find('\n').map_or(0, |n| n + 1);
    let Some(close) = after[body_start..].find("```") else {
        return (Some(after[body_start..].trim().to_owned()), markdown[..open].trim().to_owned());
    };
    let signature = after[body_start..body_start + close].trim().to_owned();
    let rest_start = open + 3 + body_start + close + 3;
    let docs = format!("{}\n{}", &markdown[..open], &markdown[rest_start..]);
    (Some(signature), docs.trim().to_owned())
}

#[async_trait::async_trait]
impl VizierTool for LspHover {
    type Input = PositionInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "lsp_hover".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn description(&self) -> String {
        "Get a symbol's type signature and documentation from the language server. Name the symbol \
         and the line it is on. Returns `signature` and `docs` (cut at 4 KiB)."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        Ok(match position_request(ctx, &args, "textDocument/hover", None).await {
            Ok((_ready, answer, _enc)) => {
                let markdown = answer.get("contents").map(text_of).unwrap_or_default();
                if markdown.trim().is_empty() {
                    serde_json::json!({ "status": "ok", "signature": null, "docs": null })
                } else {
                    let (signature, mut docs) = split_signature(&markdown);
                    if docs.len() > DOCS_CAP {
                        let mut cut = DOCS_CAP;
                        while !docs.is_char_boundary(cut) {
                            cut -= 1;
                        }
                        docs.truncate(cut);
                        docs.push('…');
                    }
                    serde_json::json!({ "status": "ok", "signature": signature, "docs": docs })
                }
            }
            Err(status) => status,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fenced_signature_is_split_from_its_docs() {
        let (signature, docs) = split_signature("```rust\npub fn resolve(cwd: &Path)\n```\n\nResolves a path.");
        assert_eq!(signature.as_deref(), Some("pub fn resolve(cwd: &Path)"));
        assert_eq!(docs, "Resolves a path.");
    }

    #[test]
    fn plain_text_uses_the_first_line() {
        let (signature, docs) = split_signature("fn a()\nmore text");
        assert_eq!(signature.as_deref(), Some("fn a()"));
        assert_eq!(docs, "more text");
    }

    #[test]
    fn contents_of_every_shape() {
        assert_eq!(text_of(&serde_json::json!("x")), "x");
        assert_eq!(text_of(&serde_json::json!({"kind":"markdown","value":"m"})), "m");
        assert_eq!(text_of(&serde_json::json!({"language":"rust","value":"fn a()"})), "```rust\nfn a()\n```");
        assert_eq!(text_of(&serde_json::json!(["a", {"value": "b"}])), "a\n\nb");
    }
}
