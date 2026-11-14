//! Input schemas (`specs/061-agent-custom-tools` research R6): a documented JSON Schema subset,
//! checked by hand. At save, [`check_schema`] refuses anything outside the subset with the
//! keyword and its path; at call, [`validate`] reports every mismatch with its path and fills
//! `default`s before the script runs.

use serde_json::{Map, Value};

use super::Problem;

const KEYWORDS: &[&str] = &[
    "type",
    "properties",
    "required",
    "additionalProperties",
    "items",
    "enum",
    "const",
    "description",
    "default",
    "title",
    "minimum",
    "maximum",
    "minLength",
    "maxLength",
    "minItems",
    "maxItems",
];

const TYPES: &[&str] = &["object", "string", "number", "integer", "boolean", "array", "null"];

/// Every problem with a schema, at once. The top level must be an object schema.
pub fn check_schema(schema: &Value) -> Vec<Problem> {
    let mut problems = vec![];
    let top_is_object = schema.get("type").and_then(Value::as_str) == Some("object");
    if !schema.is_object() || !top_is_object {
        problems.push(Problem::new(
            "schema_not_object",
            "input_schema must be an object schema: {\"type\": \"object\", \"properties\": {…}}",
        ));
    }
    if schema.is_object() {
        check_node(schema, "$", &mut problems);
    }
    problems
}

fn check_node(node: &Value, path: &str, problems: &mut Vec<Problem>) {
    let Some(map) = node.as_object() else {
        problems.push(
            Problem::new("schema_invalid", format!("{path}: a schema must be an object"))
                .with("path", path),
        );
        return;
    };
    for (keyword, value) in map {
        if !KEYWORDS.contains(&keyword.as_str()) {
            problems.push(
                Problem::new(
                    "schema_keyword_unsupported",
                    format!("{path}: `{keyword}` is not supported in a custom tool's input schema"),
                )
                .with("path", path)
                .with("keyword", keyword.as_str()),
            );
            continue;
        }
        match keyword.as_str() {
            "type" => {
                let names: Vec<&Value> = match value {
                    Value::Array(list) => list.iter().collect(),
                    other => vec![other],
                };
                for name in names {
                    if !name.as_str().is_some_and(|name| TYPES.contains(&name)) {
                        problems.push(
                            Problem::new("schema_invalid", format!("{path}: unknown type {name}"))
                                .with("path", path),
                        );
                    }
                }
            }
            "properties" => match value.as_object() {
                Some(properties) => {
                    for (name, child) in properties {
                        check_node(child, &format!("{path}.properties.{name}"), problems);
                    }
                }
                None => problems.push(
                    Problem::new("schema_invalid", format!("{path}: properties must be an object"))
                        .with("path", path),
                ),
            },
            "items" => check_node(value, &format!("{path}.items"), problems),
            "required" if !value.as_array().is_some_and(|list| list.iter().all(Value::is_string)) => {
                problems.push(
                    Problem::new("schema_invalid", format!("{path}: required must be a list of names"))
                        .with("path", path),
                )
            }
            "additionalProperties" if !value.is_boolean() => problems.push(
                Problem::new(
                    "schema_keyword_unsupported",
                    format!("{path}: additionalProperties must be true or false"),
                )
                .with("path", path)
                .with("keyword", "additionalProperties"),
            ),
            "enum" if !value.is_array() => problems.push(
                Problem::new("schema_invalid", format!("{path}: enum must be a list"))
                    .with("path", path),
            ),
            "minimum" | "maximum" if !value.is_number() => problems.push(
                Problem::new("schema_invalid", format!("{path}: {keyword} must be a number"))
                    .with("path", path),
            ),
            "minLength" | "maxLength" | "minItems" | "maxItems" if !value.is_u64() => problems.push(
                Problem::new("schema_invalid", format!("{path}: {keyword} must be a whole number"))
                    .with("path", path),
            ),
            _ => {}
        }
    }
}

/// `input` checked against `schema`, with `default`s filled in, or every mismatch. A missing
/// input is an empty object.
pub fn validate(schema: &Value, input: &Value) -> Result<Value, Vec<String>> {
    let mut value = if input.is_null() { Value::Object(Map::new()) } else { input.clone() };
    let mut errors = vec![];
    check_value(schema, &mut value, "$", &mut errors);
    if errors.is_empty() { Ok(value) } else { Err(errors) }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn matches_type(expected: &str, value: &Value) -> bool {
    match expected {
        "number" => value.is_number(),
        "integer" => {
            value.is_i64()
                || value.is_u64()
                || value.as_f64().is_some_and(|n| n.fract() == 0.0 && n.is_finite())
        }
        other => type_name(value) == other,
    }
}

fn check_value(schema: &Value, value: &mut Value, path: &str, errors: &mut Vec<String>) {
    let Some(schema) = schema.as_object() else {
        return;
    };
    if let Some(expected) = schema.get("type") {
        let names: Vec<&str> = match expected {
            Value::Array(list) => list.iter().filter_map(Value::as_str).collect(),
            other => other.as_str().into_iter().collect(),
        };
        if !names.is_empty() && !names.iter().any(|name| matches_type(name, value)) {
            errors.push(format!("{path}: expected {}, got {}", names.join(" or "), type_name(value)));
            return;
        }
    }
    if let Some(options) = schema.get("enum").and_then(Value::as_array) {
        if !options.contains(value) {
            let shown: Vec<String> = options.iter().map(Value::to_string).collect();
            errors.push(format!("{path}: must be one of {}", shown.join(", ")));
        }
    }
    if let Some(constant) = schema.get("const") {
        if constant != value {
            errors.push(format!("{path}: must be {constant}"));
        }
    }
    if let Some(n) = value.as_f64() {
        if let Some(min) = schema.get("minimum").and_then(Value::as_f64) {
            if n < min {
                errors.push(format!("{path}: must be at least {min}"));
            }
        }
        if let Some(max) = schema.get("maximum").and_then(Value::as_f64) {
            if n > max {
                errors.push(format!("{path}: must be at most {max}"));
            }
        }
    }
    if let Some(text) = value.as_str() {
        let length = text.chars().count() as u64;
        if let Some(min) = schema.get("minLength").and_then(Value::as_u64) {
            if length < min {
                errors.push(format!("{path}: must be at least {min} characters"));
            }
        }
        if let Some(max) = schema.get("maxLength").and_then(Value::as_u64) {
            if length > max {
                errors.push(format!("{path}: must be at most {max} characters"));
            }
        }
    }
    match value {
        Value::Array(items) => {
            let count = items.len() as u64;
            if let Some(min) = schema.get("minItems").and_then(Value::as_u64) {
                if count < min {
                    errors.push(format!("{path}: must have at least {min} items"));
                }
            }
            if let Some(max) = schema.get("maxItems").and_then(Value::as_u64) {
                if count > max {
                    errors.push(format!("{path}: must have at most {max} items"));
                }
            }
            if let Some(item_schema) = schema.get("items") {
                for (index, item) in items.iter_mut().enumerate() {
                    check_value(item_schema, item, &format!("{path}[{index}]"), errors);
                }
            }
        }
        Value::Object(map) => {
            let properties = schema.get("properties").and_then(Value::as_object);
            if let Some(properties) = properties {
                for (name, child) in properties {
                    if !map.contains_key(name) {
                        if let Some(default) = child.get("default") {
                            map.insert(name.clone(), default.clone());
                        }
                    }
                }
            }
            if let Some(required) = schema.get("required").and_then(Value::as_array) {
                for name in required.iter().filter_map(Value::as_str) {
                    if !map.contains_key(name) {
                        errors.push(format!("{path}.{name}: is required"));
                    }
                }
            }
            if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
                for name in map.keys() {
                    if !properties.is_some_and(|properties| properties.contains_key(name)) {
                        errors.push(format!("{path}.{name}: is not an accepted argument"));
                    }
                }
            }
            if let Some(properties) = properties {
                for (name, child) in properties {
                    if let Some(item) = map.get_mut(name) {
                        check_value(child, item, &format!("{path}.{name}"), errors);
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn codes(schema: Value) -> Vec<String> {
        check_schema(&schema).into_iter().map(|p| p.code).collect()
    }

    #[test]
    fn the_top_level_must_be_an_object_schema() {
        assert_eq!(codes(json!({"type": "string"})), vec!["schema_not_object"]);
        assert!(codes(json!({"type": "object", "properties": {"a": {"type": "integer"}}})).is_empty());
    }

    #[test]
    fn an_unsupported_keyword_names_itself_and_its_path() {
        let problems = check_schema(&json!({
            "type": "object",
            "properties": {"when": {"type": "string", "format": "date"}, "tags": {"type": "array", "items": {"pattern": "x"}}}
        }));
        let mut paths: Vec<_> = problems.iter().map(|p| p.extra["path"].as_str().unwrap().to_string()).collect();
        paths.sort();
        assert_eq!(paths, vec!["$.properties.tags.items", "$.properties.when"]);
        assert!(problems.iter().all(|p| p.code == "schema_keyword_unsupported"));
    }

    #[test]
    fn every_mismatch_is_reported_with_its_path() {
        let schema = json!({
            "type": "object",
            "properties": {
                "a": {"type": "integer"},
                "items": {"type": "array", "items": {"type": "object", "properties": {"id": {"type": "string"}}}},
                "mode": {"enum": ["fast", "slow"]}
            },
            "required": ["a", "b"]
        });
        let errors = validate(&schema, &json!({"a": "2", "items": [{"id": "x"}, {"id": 3}], "mode": "odd"})).unwrap_err();
        assert!(errors.contains(&"$.a: expected integer, got string".to_string()));
        assert!(errors.contains(&"$.b: is required".to_string()));
        assert!(errors.contains(&"$.items[1].id: expected string, got integer".to_string()));
        assert!(errors.iter().any(|e| e.starts_with("$.mode: must be one of")));
    }

    #[test]
    fn defaults_are_filled_and_limits_checked() {
        let schema = json!({
            "type": "object",
            "properties": {"limit": {"type": "integer", "default": 10, "minimum": 1, "maximum": 50}},
            "additionalProperties": false
        });
        assert_eq!(validate(&schema, &json!({})).unwrap(), json!({"limit": 10}));
        assert_eq!(validate(&schema, &Value::Null).unwrap(), json!({"limit": 10}));
        let errors = validate(&schema, &json!({"limit": 99, "extra": 1})).unwrap_err();
        assert_eq!(errors.len(), 2);
    }
}
