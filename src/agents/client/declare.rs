//! Client tool declarations, as a WebSocket client sends them on a request, and the shape
//! check that runs before the request reaches the agent (specs/019-client-tools research R3).
//!
//! "Valid JSON Schema" is checked structurally: the crate tree has no JSON Schema validator,
//! and MCP schemas are already handed to the model unvalidated. Arguments the model sends are
//! not checked against the schema either; the client owns the tool, so the client validates.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// The category a tool declaring none is listed under.
pub const DEFAULT_CATEGORY: &str = "client";
/// A call's time limit when the declaration sets none.
pub const DEFAULT_TIMEOUT_SECS: u64 = 60;
/// The most a declaration may ask for.
pub const MAX_TIMEOUT_SECS: u64 = 600;

const MAX_NAME_BYTES: usize = 64;
const MAX_TOOLS: usize = 32;
const MAX_CATEGORIES: usize = 16;
const MAX_DESCRIPTION_BYTES: usize = 2048;
const MAX_CATEGORY_DESCRIPTION_BYTES: usize = 512;
const MAX_SCHEMA_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientToolDeclaration {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub input_schema: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

impl ClientToolDeclaration {
    /// The declared category, or `client`.
    pub fn category(&self) -> &str {
        self.category.as_deref().unwrap_or(DEFAULT_CATEGORY)
    }

    /// The call's time limit: the declared one, or the default, never above the maximum.
    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(
            self.timeout_secs
                .unwrap_or(DEFAULT_TIMEOUT_SECS)
                .min(MAX_TIMEOUT_SECS),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientCategoryDeclaration {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Why a declaration was refused: `tool` is `"category/name"` (or the category's name for a
/// category rule), `rule` one of the protocol's rule names.
#[derive(Debug, Clone, PartialEq)]
pub struct DeclarationError {
    pub tool: String,
    pub rule: &'static str,
    pub message: String,
}

impl DeclarationError {
    fn new(tool: impl Into<String>, rule: &'static str, message: impl Into<String>) -> Self {
        Self {
            tool: tool.into(),
            rule,
            message: message.into(),
        }
    }

    /// The `client_error` frame refusing the request.
    pub fn frame(&self) -> serde_json::Value {
        serde_json::json!({
            "client_error": {
                "code": "invalid_client_tools",
                "tool": self.tool,
                "rule": self.rule,
                "message": self.message,
            }
        })
    }
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && name.len() <= MAX_NAME_BYTES
}

fn schema_size(schema: &serde_json::Value) -> usize {
    serde_json::to_string(schema).map(|s| s.len()).unwrap_or(usize::MAX)
}

/// Check every rule of the protocol's §1, in declaration order, and report the first broken.
pub fn validate(
    tools: &[ClientToolDeclaration],
    categories: &[ClientCategoryDeclaration],
) -> Result<(), DeclarationError> {
    if tools.len() > MAX_TOOLS {
        return Err(DeclarationError::new(
            "",
            "too_many_tools",
            format!("{} tools declared; the limit is {MAX_TOOLS}", tools.len()),
        ));
    }

    let mut seen = BTreeSet::new();
    let mut used = BTreeSet::new();
    for tool in tools {
        let category = tool.category();
        let id = format!("{category}/{}", tool.name);

        if !is_identifier(&tool.name) {
            return Err(DeclarationError::new(
                id,
                "name_pattern",
                format!(
                    "name '{}' must match ^[a-z][a-z0-9_]*$ and be at most {MAX_NAME_BYTES} bytes",
                    tool.name
                ),
            ));
        }
        if !is_identifier(category) {
            return Err(DeclarationError::new(
                id,
                "name_pattern",
                format!(
                    "category '{category}' must match ^[a-z][a-z0-9_]*$ and be at most {MAX_NAME_BYTES} bytes"
                ),
            ));
        }
        if !seen.insert((category.to_string(), tool.name.clone())) {
            return Err(DeclarationError::new(
                id,
                "duplicate_name",
                format!("'{}' is declared twice in category '{category}'", tool.name),
            ));
        }
        used.insert(category.to_string());
        if used.len() > MAX_CATEGORIES {
            return Err(DeclarationError::new(
                id,
                "too_many_categories",
                format!("more than {MAX_CATEGORIES} categories declared"),
            ));
        }

        if tool.description.trim().is_empty() {
            return Err(DeclarationError::new(id, "description_missing", "description is empty"));
        }
        if tool.description.len() > MAX_DESCRIPTION_BYTES {
            return Err(DeclarationError::new(
                id,
                "description_too_long",
                format!(
                    "description is {} bytes; the limit is {MAX_DESCRIPTION_BYTES}",
                    tool.description.len()
                ),
            ));
        }

        let input_is_object = tool.input_schema.is_object()
            && tool.input_schema.get("type").and_then(|t| t.as_str()) == Some("object");
        if !input_is_object {
            return Err(DeclarationError::new(
                id,
                "schema_not_object",
                "input_schema must be a JSON Schema object with \"type\": \"object\"",
            ));
        }
        if let Some(output) = &tool.output_schema
            && !output.is_object()
        {
            return Err(DeclarationError::new(
                id,
                "schema_not_object",
                "output_schema must be a JSON Schema object",
            ));
        }
        for (which, schema) in [("input_schema", Some(&tool.input_schema)), ("output_schema", tool.output_schema.as_ref())] {
            if let Some(schema) = schema {
                let size = schema_size(schema);
                if size > MAX_SCHEMA_BYTES {
                    return Err(DeclarationError::new(
                        id,
                        "schema_too_large",
                        format!("{which} is {size} bytes; the limit is {MAX_SCHEMA_BYTES}"),
                    ));
                }
            }
        }

        if let Some(secs) = tool.timeout_secs
            && !(1..=MAX_TIMEOUT_SECS).contains(&secs)
        {
            return Err(DeclarationError::new(
                id,
                "timeout_out_of_range",
                format!("timeout_secs is {secs}; it must be 1 to {MAX_TIMEOUT_SECS}"),
            ));
        }
    }

    // A description for a category no tool uses is ignored, so only used ones are checked.
    for category in categories.iter().filter(|c| used.contains(&c.name)) {
        if let Some(description) = &category.description
            && description.len() > MAX_CATEGORY_DESCRIPTION_BYTES
        {
            return Err(DeclarationError::new(
                category.name.clone(),
                "category_description_too_long",
                format!(
                    "category description is {} bytes; the limit is {MAX_CATEGORY_DESCRIPTION_BYTES}",
                    description.len()
                ),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn tool(name: &str) -> ClientToolDeclaration {
        ClientToolDeclaration {
            name: name.into(),
            category: None,
            description: "does a thing".into(),
            input_schema: json!({ "type": "object" }),
            output_schema: None,
            timeout_secs: None,
        }
    }

    fn rule(tools: &[ClientToolDeclaration], categories: &[ClientCategoryDeclaration]) -> &'static str {
        validate(tools, categories).unwrap_err().rule
    }

    #[test]
    fn a_good_declaration_passes_and_defaults_to_the_client_category() {
        let t = tool("pick_color");
        assert_eq!(validate(&[t.clone()], &[]), Ok(()));
        assert_eq!(t.category(), "client");
        assert_eq!(t.timeout(), std::time::Duration::from_secs(60));
    }

    #[test]
    fn names_and_categories_follow_the_pattern() {
        assert_eq!(rule(&[tool("Bad-Name")], &[]), "name_pattern");
        assert_eq!(rule(&[tool("1abc")], &[]), "name_pattern");
        assert_eq!(rule(&[tool(&"a".repeat(65))], &[]), "name_pattern");
        let mut t = tool("ok");
        t.category = Some("Pal ette".into());
        assert_eq!(rule(&[t], &[]), "name_pattern");
        assert_eq!(validate(&[tool(&"a".repeat(64))], &[]), Ok(()));
    }

    #[test]
    fn the_error_names_the_tool_as_category_slash_name() {
        let mut t = tool("pick");
        t.category = Some("palette".into());
        t.description = "x".repeat(3000);
        let err = validate(&[t], &[]).unwrap_err();
        assert_eq!(err.tool, "palette/pick");
        assert_eq!(err.rule, "description_too_long");
        assert_eq!(err.frame()["client_error"]["code"], "invalid_client_tools");
    }

    #[test]
    fn duplicates_are_refused_but_the_same_name_in_two_categories_is_not() {
        assert_eq!(rule(&[tool("a"), tool("a")], &[]), "duplicate_name");
        let mut other = tool("a");
        other.category = Some("other".into());
        assert_eq!(validate(&[tool("a"), other], &[]), Ok(()));
    }

    #[test]
    fn counts_are_limited() {
        let many: Vec<_> = (0..33).map(|i| tool(&format!("t{i}"))).collect();
        assert_eq!(rule(&many, &[]), "too_many_tools");
        let spread: Vec<_> = (0..17)
            .map(|i| {
                let mut t = tool("t");
                t.category = Some(format!("c{i}"));
                t
            })
            .collect();
        assert_eq!(rule(&spread, &[]), "too_many_categories");
    }

    #[test]
    fn descriptions_are_required_and_limited() {
        let mut t = tool("a");
        t.description = "  ".into();
        assert_eq!(rule(&[t], &[]), "description_missing");
        let long = ClientCategoryDeclaration {
            name: "client".into(),
            description: Some("x".repeat(513)),
        };
        assert_eq!(rule(&[tool("a")], &[long]), "category_description_too_long");
    }

    #[test]
    fn a_description_for_an_unused_category_is_ignored() {
        let unused = ClientCategoryDeclaration {
            name: "nobody".into(),
            description: Some("x".repeat(10_000)),
        };
        assert_eq!(validate(&[tool("a")], &[unused]), Ok(()));
    }

    #[test]
    fn schemas_must_be_objects_within_the_size_limit() {
        let mut t = tool("a");
        t.input_schema = json!({ "type": "string" });
        assert_eq!(rule(&[t.clone()], &[]), "schema_not_object");
        t.input_schema = json!([1]);
        assert_eq!(rule(&[t.clone()], &[]), "schema_not_object");
        t.input_schema = json!({ "type": "object" });
        t.output_schema = Some(json!("nope"));
        assert_eq!(rule(&[t.clone()], &[]), "schema_not_object");
        t.output_schema = None;
        t.input_schema = json!({ "type": "object", "description": "x".repeat(17_000) });
        assert_eq!(rule(&[t], &[]), "schema_too_large");
    }

    #[test]
    fn timeouts_are_one_to_six_hundred_seconds() {
        let mut t = tool("a");
        t.timeout_secs = Some(0);
        assert_eq!(rule(&[t.clone()], &[]), "timeout_out_of_range");
        t.timeout_secs = Some(601);
        assert_eq!(rule(&[t.clone()], &[]), "timeout_out_of_range");
        t.timeout_secs = Some(600);
        assert_eq!(validate(&[t], &[]), Ok(()));
    }
}
