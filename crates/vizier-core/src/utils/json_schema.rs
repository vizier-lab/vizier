//! Generic JSON Schema helpers over plain `serde_json::Value`s: resolving local `$ref`s,
//! listing an object schema's top-level properties, and generating a placeholder value that
//! conforms to a schema. Not tied to tools; covers what schemars emits plus typical MCP schemas.

use serde_json::{Map, Value, json};

/// Nesting limit for [`sample_value`]; protects against recursive `$ref`s.
const MAX_DEPTH: u8 = 8;

/// A top-level property of an object schema.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaProperty {
    pub name: String,
    /// The property's own `description`, else its `$ref` target's, folded to one line.
    pub description: Option<String>,
    /// Whether the name is listed in the schema's `required`.
    pub required: bool,
    /// The property's (ref-resolved) sub-schema.
    pub schema: Value,
}

/// Resolve a local `$ref` (`#`, `#/$defs/X`, `#/definitions/X`, or any other `#/` pointer)
/// against `root`. Non-ref nodes, and refs whose target is missing, are returned as-is.
pub fn resolve<'a>(node: &'a Value, root: &'a Value) -> &'a Value {
    match node.get("$ref").and_then(Value::as_str) {
        Some("#") => root,
        Some(pointer) if pointer.starts_with("#/") => root.pointer(&pointer[1..]).unwrap_or(node),
        _ => node,
    }
}

/// Top-level properties of an object schema, following `$ref` and merging `allOf` branches.
///
/// The crate builds `serde_json` without `preserve_order`, so `properties` is a `BTreeMap` and
/// declared key order is already lost. Required properties therefore come first in the order of
/// the `required` array (which schemars emits in field order), then optional ones alphabetically.
pub fn properties(schema: &Value) -> Vec<SchemaProperty> {
    let root = resolve(schema, schema);
    let (props, required) = merged_object(root, schema);

    let mut result = Vec::with_capacity(props.len());
    for name in &required {
        if let Some(prop) = props.get(name) {
            result.push(property(name, prop, true, schema));
        }
    }
    let mut optional = props
        .iter()
        .filter(|(name, _)| !required.contains(name))
        .collect::<Vec<_>>();
    optional.sort_by(|a, b| a.0.cmp(b.0));
    for (name, prop) in optional {
        result.push(property(name, prop, false, schema));
    }
    result
}

/// A placeholder value that conforms to `schema` in type and shape.
pub fn sample_value(schema: &Value) -> Value {
    sample(schema, schema, None, 0)
}

fn property(name: &str, prop: &Value, required: bool, root: &Value) -> SchemaProperty {
    let resolved = resolve(prop, root);
    let description = prop
        .get("description")
        .or_else(|| resolved.get("description"))
        .and_then(Value::as_str)
        .map(|d| d.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|d| !d.is_empty());
    SchemaProperty {
        name: name.to_string(),
        description,
        required,
        schema: resolved.clone(),
    }
}

/// `properties` and `required` of an object schema, merged across its `allOf` branches.
fn merged_object(node: &Value, root: &Value) -> (Map<String, Value>, Vec<String>) {
    let mut props = Map::new();
    let mut required = Vec::<String>::new();
    let branches = node
        .get("allOf")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|branch| resolve(branch, root));
    for part in std::iter::once(node).chain(branches) {
        if let Some(p) = part.get("properties").and_then(Value::as_object) {
            for (name, schema) in p {
                props.entry(name.clone()).or_insert_with(|| schema.clone());
            }
        }
        for name in part
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !required.iter().any(|r| r == name) {
                required.push(name.to_string());
            }
        }
    }
    (props, required)
}

fn is_null_schema(node: &Value) -> bool {
    node.get("type").and_then(Value::as_str) == Some("null")
}

fn sample(node: &Value, root: &Value, name_hint: Option<&str>, depth: u8) -> Value {
    if depth > MAX_DEPTH {
        return Value::Null;
    }
    let Some(obj) = node.as_object() else {
        return Value::Null;
    };

    // 1. Explicit values.
    if let Some(value) = obj
        .get("default")
        .or_else(|| obj.get("examples").and_then(|e| e.get(0)))
        .or_else(|| obj.get("example"))
    {
        return value.clone();
    }
    // 2. Constants and enums.
    if let Some(value) = obj
        .get("const")
        .or_else(|| obj.get("enum").and_then(|e| e.get(0)))
    {
        return value.clone();
    }
    // 3. Local references.
    if obj.contains_key("$ref") {
        let target = resolve(node, root);
        if std::ptr::eq(target, node) {
            return Value::Null;
        }
        return sample(target, root, name_hint, depth + 1);
    }
    // 4. Combinators.
    if let Some(branches) = obj
        .get("anyOf")
        .or_else(|| obj.get("oneOf"))
        .and_then(Value::as_array)
    {
        return branches
            .iter()
            .find(|branch| !is_null_schema(resolve(branch, root)))
            .map(|branch| sample(branch, root, name_hint, depth + 1))
            .unwrap_or(Value::Null);
    }
    if obj.contains_key("allOf") {
        let (props, _) = merged_object(node, root);
        if props.is_empty() {
            // e.g. `allOf: [{"$ref": ...}]` wrapping a non-object type
            if let Some(first) = obj.get("allOf").and_then(|a| a.get(0)) {
                return sample(first, root, name_hint, depth + 1);
            }
        }
        return sample(
            &json!({ "type": "object", "properties": props }),
            root,
            name_hint,
            depth + 1,
        );
    }

    // 5. Types.
    let ty = match obj.get("type") {
        Some(Value::String(t)) => Some(t.as_str()),
        Some(Value::Array(types)) => types
            .iter()
            .filter_map(Value::as_str)
            .find(|t| *t != "null"),
        _ if obj.contains_key("properties") => Some("object"),
        _ => None,
    };
    match ty {
        Some("string") => Value::String(name_hint.unwrap_or("string").to_string()),
        Some("integer") => json!(0),
        Some("number") => json!(0.0),
        Some("boolean") => Value::Bool(false),
        Some("array") => match obj.get("items") {
            // An item that bottoms out at the depth limit becomes an empty array, so recursive
            // types still produce a value that deserializes.
            Some(items) => match sample(items, root, name_hint, depth + 1) {
                Value::Null => json!([]),
                item => json!([item]),
            },
            None => json!([]),
        },
        Some("object") => Value::Object(
            obj.get("properties")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .map(|(name, prop)| (name.clone(), sample(prop, root, Some(name), depth + 1)))
                .collect(),
        ),
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use schemars::{JsonSchema, schema_for};
    use serde::{Deserialize, de::DeserializeOwned};

    use super::*;

    #[derive(JsonSchema, Deserialize)]
    struct NoArgs {}

    #[derive(JsonSchema, Deserialize)]
    struct WithOption {
        a: String,
        b: Option<u32>,
    }

    #[derive(JsonSchema, Deserialize)]
    #[allow(dead_code)]
    struct Ordered {
        z: String,
        y: Option<String>,
        a: String,
        b: Option<String>,
    }

    /// The inner type's own docs.
    #[derive(JsonSchema, Deserialize)]
    struct Inner {
        x: bool,
    }

    #[derive(JsonSchema, Deserialize)]
    struct Nested {
        inner: Inner,
    }

    #[derive(JsonSchema, Deserialize)]
    struct WithVec {
        items: Vec<String>,
    }

    #[derive(JsonSchema, Deserialize, PartialEq, Debug)]
    enum Mode {
        Fast,
        Slow,
    }

    #[derive(JsonSchema, Deserialize)]
    struct WithEnum {
        mode: Mode,
    }

    fn default_count() -> u32 {
        7
    }

    #[derive(JsonSchema, Deserialize)]
    struct WithDefault {
        #[serde(default = "default_count")]
        count: u32,
    }

    #[derive(JsonSchema, Deserialize)]
    struct Node {
        children: Vec<Node>,
    }

    #[derive(JsonSchema, Deserialize)]
    #[allow(dead_code)]
    struct Documented {
        /// The title
        /// of the thing.
        title: String,
        /// Tags used to group things.
        tags: Vec<String>,
        inner: Inner,
        plain: u32,
    }

    fn schema<T: JsonSchema>() -> Value {
        serde_json::to_value(schema_for!(T)).unwrap()
    }

    fn round_trip<T: JsonSchema + DeserializeOwned>() -> T {
        let sample = sample_value(&schema::<T>());
        serde_json::from_value::<T>(sample.clone())
            .unwrap_or_else(|e| panic!("sample {sample} does not deserialize: {e}"))
    }

    #[test]
    fn samples_deserialize_back_into_their_types() {
        round_trip::<NoArgs>();
        round_trip::<WithOption>();
        round_trip::<Ordered>();
        round_trip::<Nested>();
        round_trip::<WithVec>();
        round_trip::<Documented>();
        assert_eq!(round_trip::<WithEnum>().mode, Mode::Fast);
        assert_eq!(round_trip::<WithDefault>().count, 7);
    }

    #[test]
    fn recursive_types_terminate() {
        let sample = sample_value(&schema::<Node>());
        assert!(sample.get("children").is_some());
        // Bottoming out as `[]` keeps even the recursive sample valid.
        round_trip::<Node>();
    }

    #[test]
    fn no_args_samples_as_an_empty_object() {
        assert_eq!(sample_value(&schema::<NoArgs>()), json!({}));
        assert!(properties(&schema::<NoArgs>()).is_empty());
    }

    #[test]
    fn option_samples_as_the_inner_type_and_vec_as_one_item() {
        assert_eq!(
            sample_value(&schema::<WithOption>()),
            json!({"a": "a", "b": 0})
        );
        assert_eq!(
            sample_value(&schema::<WithVec>()),
            json!({"items": ["items"]})
        );
    }

    #[test]
    fn required_come_first_in_order_then_optional_alphabetically() {
        let props = properties(&schema::<WithOption>());
        assert_eq!(
            props
                .iter()
                .map(|p| (p.name.as_str(), p.required))
                .collect::<Vec<_>>(),
            vec![("a", true), ("b", false)]
        );

        let names = properties(&schema::<Ordered>())
            .into_iter()
            .map(|p| p.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["z", "a", "b", "y"]);
    }

    #[test]
    fn descriptions_are_folded_and_fall_back_to_the_ref_target() {
        let props = properties(&schema::<Documented>());
        let desc = |name: &str| {
            props
                .iter()
                .find(|p| p.name == name)
                .and_then(|p| p.description.clone())
        };
        assert_eq!(desc("title").as_deref(), Some("The title of the thing."));
        assert_eq!(desc("tags").as_deref(), Some("Tags used to group things."));
        assert_eq!(desc("inner").as_deref(), Some("The inner type's own docs."));
        assert_eq!(desc("plain"), None);
    }

    #[test]
    fn all_of_branches_are_merged() {
        let schema = json!({
            "allOf": [
                {"type": "object", "properties": {"a": {"type": "string"}}, "required": ["a"]},
                {"type": "object", "properties": {"b": {"type": "integer"}}}
            ]
        });
        let props = properties(&schema);
        assert_eq!(props.len(), 2);
        assert!(props[0].required && props[0].name == "a");
        assert_eq!(sample_value(&schema), json!({"a": "a", "b": 0}));
    }

    #[test]
    fn hand_written_mcp_schema_with_nullable_type_samples_as_string() {
        let schema = json!({
            "type": "object",
            "properties": {
                "path": {"type": ["string", "null"], "description": "File path"}
            },
            "required": ["path"]
        });
        assert_eq!(sample_value(&schema), json!({"path": "path"}));
        assert_eq!(
            properties(&schema)[0].description.as_deref(),
            Some("File path")
        );
    }
}
