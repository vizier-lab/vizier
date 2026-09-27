//! Script-callable function documentation, derived from `ToolDefinition` — the
//! same definitions the model receives, so there is no second source of truth.

use std::collections::HashSet;

use rig_core::completion::ToolDefinition;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct ToolFunctionDoc {
    /// Sanitised Python identifier (== tool name for all native tools).
    pub function: String,
    /// Original tool name (differs only if sanitised).
    pub tool: String,
    /// First sentence/line of the tool description.
    pub summary: String,
    pub description: String,
    pub parameters: Vec<ParamDoc>,
    /// Rendered output schema, or "any JSON value".
    pub returns: String,
    /// Generated: `result = name(required_a="…", required_b=0)`.
    pub example: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParamDoc {
    pub name: String,
    pub r#type: String,
    pub required: bool,
    pub description: String,
}

/// Names a script cannot use for a tool function: the in-script helpers,
/// Python keywords, and the builtins a script is most likely to need.
const RESERVED: &[&str] = &[
    "list_tools", "describe_tool", "execute_python",
    // keywords
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
    "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
    "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return",
    "try", "while", "with", "yield",
    // builtins
    "abs", "all", "any", "bool", "bytes", "callable", "chr", "dict", "dir", "divmod",
    "enumerate", "filter", "float", "format", "frozenset", "getattr", "hasattr", "hash", "id",
    "input", "int", "isinstance", "issubclass", "iter", "len", "list", "map", "max", "min",
    "next", "object", "open", "ord", "pow", "print", "range", "repr", "reversed", "round",
    "set", "setattr", "sorted", "str", "sum", "super", "tuple", "type", "vars", "zip",
];

/// A valid, non-reserved Python identifier for a tool name.
pub fn python_identifier(tool_name: &str) -> String {
    let mut ident: String = tool_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if ident.is_empty() || ident.starts_with(|c: char| c.is_ascii_digit()) {
        ident.insert(0, '_');
    }
    if RESERVED.contains(&ident.as_str()) {
        ident.push_str("_tool");
    }
    ident
}

/// `(function, definition)` pairs with collisions resolved deterministically:
/// original names in sorted order, later duplicates suffixed `_2`, `_3`, …
fn named(defs: &[ToolDefinition]) -> Vec<(String, &ToolDefinition)> {
    let mut sorted: Vec<&ToolDefinition> = defs.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    let mut taken = HashSet::new();
    let mut out = Vec::with_capacity(sorted.len());
    for def in sorted {
        let base = python_identifier(&def.name);
        let mut function = base.clone();
        let mut n = 2;
        while !taken.insert(function.clone()) {
            function = format!("{base}_{n}");
            n += 1;
        }
        out.push((function, def));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Every script-callable function, sorted by `function`.
pub fn catalogue(defs: &[ToolDefinition]) -> Vec<ToolFunctionDoc> {
    named(defs)
        .into_iter()
        .map(|(function, def)| describe_as(function, def, None))
        .collect()
}

/// The definition behind a script function name, with its catalogue name.
pub fn find<'a>(defs: &'a [ToolDefinition], function: &str) -> Option<&'a ToolDefinition> {
    named(defs)
        .into_iter()
        .find(|(name, _)| name == function)
        .map(|(_, def)| def)
}

/// Full documentation for one tool; `output_schema` is `None` for MCP tools.
pub fn describe(def: &ToolDefinition, output_schema: Option<&Value>) -> ToolFunctionDoc {
    describe_as(python_identifier(&def.name), def, output_schema)
}

/// `describe` for a definition whose function name was already resolved by the
/// catalogue (so collision suffixes are kept).
pub fn describe_in(
    defs: &[ToolDefinition],
    function: &str,
    output_schema: Option<&Value>,
) -> Option<ToolFunctionDoc> {
    named(defs)
        .into_iter()
        .find(|(name, _)| name == function)
        .map(|(name, def)| describe_as(name, def, output_schema))
}

fn describe_as(function: String, def: &ToolDefinition, output_schema: Option<&Value>) -> ToolFunctionDoc {
    let parameters = params(&def.parameters);
    let example_args: Vec<String> = parameters
        .iter()
        .filter(|p| p.required)
        .map(|p| format!("{}={}", p.name, placeholder(&p.r#type)))
        .collect();

    ToolFunctionDoc {
        example: format!("result = {function}({})", example_args.join(", ")),
        function,
        tool: def.name.clone(),
        summary: summary(&def.description),
        description: def.description.clone(),
        parameters,
        returns: output_schema.map_or_else(
            || "any JSON value".to_string(),
            |schema| render(schema, schema, 0),
        ),
    }
}

/// First line of the description, cut after its first sentence.
fn summary(description: &str) -> String {
    let line = description
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    match line.find(". ") {
        Some(end) => line[..=end].to_string(),
        None => line.to_string(),
    }
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Top-level parameters: required ones in declared order, then optional ones
/// alphabetically.
fn params(schema: &Value) -> Vec<ParamDoc> {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return vec![];
    };
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let doc = |name: &str, prop: &Value, required: bool| ParamDoc {
        name: name.to_string(),
        r#type: render_type(schema, prop),
        required,
        description: prop
            .get("description")
            .or_else(|| resolve_ref(schema, prop).and_then(|def| def.get("description")))
            .and_then(Value::as_str)
            .map(collapse)
            .unwrap_or_default(),
    };

    let mut out: Vec<ParamDoc> = required
        .iter()
        .filter_map(|name| properties.get(*name).map(|prop| doc(name, prop, true)))
        .collect();
    let mut optional: Vec<(&String, &Value)> = properties
        .iter()
        .filter(|(name, _)| !required.contains(&name.as_str()))
        .collect();
    optional.sort_by(|a, b| a.0.cmp(b.0));
    out.extend(optional.into_iter().map(|(name, prop)| doc(name, prop, false)));
    out
}

fn resolve_ref<'a>(root: &'a Value, schema: &Value) -> Option<&'a Value> {
    let path = schema.get("$ref")?.as_str()?.strip_prefix("#/")?;
    path.split('/').try_fold(root, |node, key| node.get(key))
}

/// A parameter's type in one short phrase: `string`, `integer | null`,
/// `list[string]`, `"a" | "b"`, or a `$ref` target's name.
fn render_type(root: &Value, schema: &Value) -> String {
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        return values.iter().map(Value::to_string).collect::<Vec<_>>().join(" | ");
    }
    if let Some(variants) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        return variants
            .iter()
            .map(|v| render_type(root, v))
            .collect::<Vec<_>>()
            .join(" | ");
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return match resolve_ref(root, schema) {
            Some(target) if target.get("enum").is_some() || target.get("type").is_some() => {
                render_type(root, target)
            }
            _ => reference.rsplit('/').next().unwrap_or(reference).to_string(),
        };
    }
    match schema.get("type") {
        Some(Value::String(t)) if t == "array" => match schema.get("items") {
            Some(items) => format!("list[{}]", render_type(root, items)),
            None => "list".into(),
        },
        Some(Value::String(t)) => t.clone(),
        Some(Value::Array(types)) => types
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" | "),
        _ => "any".into(),
    }
}

/// Compact shape of an output schema, e.g. `{ results: [{ slug: string }] }`.
fn render(root: &Value, schema: &Value, depth: usize) -> String {
    if depth > 3 {
        return "…".into();
    }
    if let Some(target) = resolve_ref(root, schema) {
        return render(root, target, depth);
    }
    if let Some(variants) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        return variants
            .iter()
            .map(|v| render(root, v, depth))
            .collect::<Vec<_>>()
            .join(" | ");
    }
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        let fields: Vec<String> = properties
            .iter()
            .map(|(name, prop)| format!("{name}: {}", render(root, prop, depth + 1)))
            .collect();
        return format!("{{ {} }}", fields.join(", "));
    }
    if schema.get("type").and_then(Value::as_str) == Some("array") {
        return match schema.get("items") {
            Some(items) => format!("[{}]", render(root, items, depth + 1)),
            None => "[]".into(),
        };
    }
    render_type(root, schema)
}

fn placeholder(r#type: &str) -> &'static str {
    match r#type.split(" | ").next().unwrap_or_default() {
        "string" => "\"…\"",
        "integer" => "0",
        "number" => "0.0",
        "boolean" => "True",
        t if t.starts_with("list") => "[]",
        "object" => "{}",
        _ => "None",
    }
}

/// The answer for a function that does not exist — data, not an error, so the
/// agent can correct itself.
pub fn not_found(function: &str, known: &[String]) -> Value {
    serde_json::json!({
        "available": false,
        "name": function,
        "message": format!("No function named '{function}' is available to this agent."),
        "did_you_mean": did_you_mean(function, known),
    })
}

/// Up to three known names close to `name` (edit distance ≤ 3, or a prefix match).
pub fn did_you_mean(name: &str, known: &[String]) -> Vec<String> {
    let mut scored: Vec<(usize, &String)> = known
        .iter()
        .filter_map(|candidate| {
            let distance = levenshtein(name, candidate);
            let prefix = candidate.starts_with(name) || name.starts_with(candidate.as_str());
            (distance <= 3 || prefix).then_some((distance, candidate))
        })
        .collect();
    scored.sort();
    scored.into_iter().take(3).map(|(_, name)| name.clone()).collect()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut current = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let substitution = prev[j] + usize::from(ca != *cb);
            current.push(substitution.min(prev[j + 1] + 1).min(current[j] + 1));
        }
        prev = current;
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn def(name: &str, description: &str, parameters: Value) -> ToolDefinition {
        ToolDefinition {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }

    #[test]
    fn identifiers_are_sanitised() {
        assert_eq!(python_identifier("my-tool"), "my_tool");
        assert_eq!(python_identifier("3d"), "_3d");
        assert_eq!(python_identifier("print"), "print_tool");
        assert_eq!(python_identifier("list_tools"), "list_tools_tool");
        assert_eq!(python_identifier("mcp_gh__create_issue"), "mcp_gh__create_issue");
    }

    #[test]
    fn collisions_get_deterministic_suffixes() {
        let defs = vec![
            def("a.b", "", json!({})),
            def("a-b", "", json!({})),
            def("a_b", "", json!({})),
        ];
        let names: Vec<(String, String)> = catalogue(&defs)
            .into_iter()
            .map(|d| (d.function, d.tool))
            .collect();
        assert_eq!(
            names,
            vec![
                ("a_b".into(), "a-b".into()),
                ("a_b_2".into(), "a.b".into()),
                ("a_b_3".into(), "a_b".into()),
            ]
        );
    }

    #[test]
    fn catalogue_is_sorted_with_first_sentence_summaries() {
        let defs = vec![
            def("zeta", "Last one. With more detail.", json!({})),
            def("alpha", "\nFirst line\nsecond line", json!({})),
        ];
        let docs = catalogue(&defs);
        assert_eq!(docs[0].function, "alpha");
        assert_eq!(docs[0].summary, "First line");
        assert_eq!(docs[1].summary, "Last one.");
    }

    #[test]
    fn describe_lists_parameters_and_builds_an_example() {
        let schema = json!({
            "type": "object",
            "required": ["query", "limit"],
            "properties": {
                "query": { "type": "string", "description": "What to  search\nfor" },
                "limit": { "type": "integer" },
                "bundle": { "type": ["string", "null"] },
                "tags": { "type": "array", "items": { "type": "string" } }
            }
        });
        let doc = describe(&def("memory_read", "Search memory.", schema), None);
        let params: Vec<(&str, &str, bool)> = doc
            .parameters
            .iter()
            .map(|p| (p.name.as_str(), p.r#type.as_str(), p.required))
            .collect();
        assert_eq!(
            params,
            vec![
                ("query", "string", true),
                ("limit", "integer", true),
                ("bundle", "string | null", false),
                ("tags", "list[string]", false),
            ]
        );
        assert_eq!(doc.parameters[0].description, "What to search for");
        assert_eq!(doc.example, "result = memory_read(query=\"…\", limit=0)");
        assert_eq!(doc.returns, "any JSON value");
    }

    #[test]
    fn returns_renders_the_output_schema() {
        let output = json!({
            "type": "object",
            "properties": { "results": { "type": "array", "items": { "$ref": "#/$defs/Hit" } } },
            "$defs": { "Hit": { "type": "object", "properties": { "slug": { "type": "string" } } } }
        });
        let doc = describe(&def("t", "", json!({})), Some(&output));
        assert_eq!(doc.returns, "{ results: [{ slug: string }] }");
    }

    #[test]
    fn did_you_mean_suggests_close_names() {
        let known: Vec<String> = ["memory_read", "memory_detail", "READ_CORE", "think"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let suggestions = did_you_mean("memory_reed", &known);
        assert_eq!(suggestions.first().map(String::as_str), Some("memory_read"));
        assert!(!suggestions.contains(&"think".to_string()));
        assert!(did_you_mean("zzzzzzzzzz", &known).is_empty());
    }
}
