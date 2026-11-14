//! Checking a definition before it is saved (`specs/061-agent-custom-tools` FR-005,
//! data-model.md "Validation"): every problem at once, so one fix-and-resave is enough.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde_json::Value;

use super::{Problem, calls, naming, schema};
use crate::storage::custom_tool::{CustomTool, CustomToolDefinition};

/// The result of a check: hard problems (nothing is saved) and warnings (saved anyway).
#[derive(Debug, Default)]
pub struct Checked {
    pub problems: Vec<Problem>,
    pub warnings: Vec<Problem>,
}

/// Check `definition` and fill its `calls`.
///
/// `known` is every non-custom function name the agent can call, or `None` when they cannot be
/// read here (the person's edit while the agent's view is unpublished): then unknown names are a
/// `tools_unchecked` warning, not a problem. `tools` is the agent's custom tools, and `editing` the
/// id of the tool being changed, if any.
pub fn validate_definition(
    definition: &mut CustomToolDefinition,
    known: Option<&HashSet<String>>,
    tools: &[CustomTool],
    editing: Option<&str>,
) -> Checked {
    let mut checked = Checked::default();
    let problems = &mut checked.problems;

    if !naming::valid_name(&definition.name) {
        problems.push(
            Problem::new(
                "name_invalid",
                format!(
                    "`{}` is not a valid name: lower-case letters, digits and _, starting with a letter",
                    definition.name
                ),
            )
            .with("name", definition.name.as_str()),
        );
    } else if tools
        .iter()
        .any(|tool| tool.name == definition.name && Some(tool.id.as_str()) != editing)
    {
        problems.push(
            Problem::new("name_taken", format!("you already have a custom tool named {}", definition.name))
                .with("name", definition.name.as_str()),
        );
    } else if let Some(why) = naming::reserved(&definition.name) {
        problems.push(
            Problem::new("name_taken", format!("{} is {why}; pick another name", definition.name))
                .with("name", definition.name.as_str()),
        );
    } else if known.is_some_and(|known| known.contains(&definition.name)) {
        // Custom tools have no prefix (spec 061 FR-011): a name any other tool has is refused.
        problems.push(
            Problem::new(
                "name_taken",
                format!("{} is already the name of one of your tools; pick another name", definition.name),
            )
            .with("name", definition.name.as_str()),
        );
    }
    if !naming::valid_name(&definition.category) {
        problems.push(
            Problem::new(
                "category_invalid",
                format!("`{}` is not a valid category name", definition.category),
            )
            .with("category", definition.category.as_str()),
        );
    }
    if definition.description.trim().is_empty() {
        problems.push(Problem::new("description_required", "a description is required"));
    }

    problems.extend(schema::check_schema(&definition.input_schema));

    match calls::parse_calls(&definition.script) {
        Ok(found) => definition.calls = found,
        Err(syntax) => {
            definition.calls = vec![];
            problems.push(
                Problem::new("script_syntax", syntax.message.clone())
                    .with("line", syntax.line)
                    .with("column", syntax.column),
            );
        }
    }

    let custom: BTreeSet<String> = tools
        .iter()
        .filter(|tool| Some(tool.id.as_str()) != editing)
        .map(|tool| tool.name.clone())
        .chain(std::iter::once(definition.name.clone()))
        .collect();
    let mut unchecked = vec![];
    for call in &definition.calls {
        if custom.contains(call) {
            continue;
        }
        match known {
            Some(known) if !known.contains(call) => {
                let mut candidates: Vec<String> = known.iter().chain(custom.iter()).cloned().collect();
                candidates.sort();
                candidates.dedup();
                problems.push(unknown(call, &candidates));
            }
            Some(_) => {}
            None => unchecked.push(call.clone()),
        }
    }
    if !unchecked.is_empty() {
        checked.warnings.push(
            Problem::new(
                "tools_unchecked",
                "the agent's tools could not be read here, so these names were not checked",
            )
            .with("names", unchecked),
        );
    }

    if let Some(path) = cycle(definition, tools, editing) {
        let shown = path.join(" → ");
        checked.problems.push(
            Problem::new("cycle", format!("cycle: {shown}")).with("path", path),
        );
    }

    if let Some(sample) = &definition.sample_input {
        if let Err(errors) = schema::validate(&definition.input_schema, sample) {
            checked.problems.push(
                Problem::new("sample_invalid", format!("sample_input does not match the schema: {}", errors.join("; ")))
                    .with("errors", errors),
            );
        }
    }
    checked
}

fn unknown(name: &str, candidates: &[String]) -> Problem {
    let mut problem = Problem::new("unknown_tool", format!("{name} is not a tool you have"))
        .with("name", name);
    let near = crate::sandbox::docs::did_you_mean(name, candidates);
    if !near.is_empty() {
        problem = problem.with("did_you_mean", near);
    }
    problem
}

/// The cycle the candidate would close through custom tools, as tool names
/// (`a → b → a`), if any.
pub fn cycle(
    definition: &CustomToolDefinition,
    tools: &[CustomTool],
    editing: Option<&str>,
) -> Option<Vec<String>> {
    let mut graph: BTreeMap<String, Vec<String>> = tools
        .iter()
        .filter(|tool| Some(tool.id.as_str()) != editing)
        .map(|tool| (tool.name.clone(), tool.current.definition.calls.clone()))
        .collect();
    graph.insert(definition.name.clone(), definition.calls.clone());
    // Only edges between custom tools count.
    let names: BTreeSet<String> = graph.keys().cloned().collect();
    for calls in graph.values_mut() {
        calls.retain(|call| names.contains(call));
    }

    let start = definition.name.clone();
    let mut path = vec![start.clone()];
    let mut seen = BTreeSet::new();
    walk(&graph, &start, &start, &mut path, &mut seen).then_some(path)
}

fn walk(
    graph: &BTreeMap<String, Vec<String>>,
    at: &str,
    target: &str,
    path: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) -> bool {
    for next in graph.get(at).into_iter().flatten() {
        if next == target {
            path.push(next.clone());
            return true;
        }
        if seen.insert(next.clone()) {
            path.push(next.clone());
            if walk(graph, next, target, path, seen) {
                return true;
            }
            path.pop();
        }
    }
    false
}

/// The other tools whose current script calls `name`.
pub fn callers(name: &str, tools: &[CustomTool]) -> Vec<String> {
    tools
        .iter()
        .filter(|tool| tool.name != name && tool.current.definition.calls.iter().any(|call| call == name))
        .map(|tool| tool.name.clone())
        .collect()
}

/// The names in `calls` that resolve to nothing: not a known function and not an enabled custom
/// tool. `None` when the known names cannot be read.
pub fn missing(calls: &[String], known: Option<&HashSet<String>>, tools: &[CustomTool]) -> Option<Vec<String>> {
    let known = known?;
    let enabled: HashSet<String> = tools
        .iter()
        .filter(|tool| tool.enabled)
        .map(|tool| tool.name.clone())
        .collect();
    Some(
        calls
            .iter()
            .filter(|call| !known.contains(*call) && !enabled.contains(*call))
            .cloned()
            .collect(),
    )
}

/// Whether a tool's calls reach a model-backed tool, directly or through other custom tools
/// (research R7).
pub fn model_backed(calls: &[String], model_backed: &HashSet<String>, tools: &[CustomTool]) -> bool {
    let mut stack: Vec<String> = calls.to_vec();
    let mut seen = HashSet::new();
    while let Some(call) = stack.pop() {
        if !seen.insert(call.clone()) {
            continue;
        }
        if model_backed.contains(&call) {
            return true;
        }
        if let Some(tool) = tools.iter().find(|tool| tool.name == call) {
            stack.extend(tool.current.definition.calls.iter().cloned());
        }
    }
    false
}

/// `missing` made transitive: a tool needs attention when a custom tool it calls does.
pub fn needs_attention(
    calls: &[String],
    known: Option<&HashSet<String>>,
    tools: &[CustomTool],
) -> Option<Vec<String>> {
    let known = known?;
    let mut missing_names = BTreeSet::new();
    let mut stack: Vec<String> = calls.to_vec();
    let mut seen = HashSet::new();
    while let Some(call) = stack.pop() {
        if !seen.insert(call.clone()) {
            continue;
        }
        // A name another tool has resolves to that tool first (the first-wins catalogue).
        if known.contains(&call) {
            continue;
        }
        match tools.iter().find(|tool| tool.name == call && tool.enabled) {
            Some(tool) => stack.extend(tool.current.definition.calls.iter().cloned()),
            None => {
                missing_names.insert(call);
            }
        }
    }
    Some(missing_names.into_iter().collect())
}

/// Whether a tool attached after this custom tool was saved has taken its name: the custom tool is
/// not offered until it is renamed. `false` when the names cannot be read.
pub fn name_taken(name: &str, known: Option<&HashSet<String>>) -> bool {
    known.is_some_and(|known| known.contains(name))
}

/// What a custom tool needs attention for: tools it calls that are gone, and its own name taken
/// by a tool attached later. `None` when nothing does or the names cannot be read.
pub fn attention(tool: &CustomTool, known: Option<&HashSet<String>>, tools: &[CustomTool]) -> Option<Value> {
    let missing = needs_attention(&tool.current.definition.calls, known, tools).unwrap_or_default();
    let taken = name_taken(&tool.name, known);
    if missing.is_empty() && !taken {
        return None;
    }
    let mut value = serde_json::json!({ "missing": missing });
    if taken {
        value["name_taken"] = Value::Bool(true);
    }
    Some(value)
}

/// A definition's schema as the description shows it, for diffs.
pub fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::storage::custom_tool::{CustomToolActor, CustomToolVersion, VersionTrigger};

    fn definition(name: &str, script: &str) -> CustomToolDefinition {
        CustomToolDefinition {
            name: name.into(),
            category: "maths".into(),
            description: "x".into(),
            input_schema: json!({"type": "object"}),
            output_description: None,
            script: script.into(),
            calls: vec![],
            sample_input: None,
        }
    }

    fn saved(name: &str, script: &str) -> CustomTool {
        let mut def = definition(name, script);
        def.calls = calls::parse_calls(script).unwrap();
        CustomTool {
            id: format!("id-{name}"),
            agent_id: "a".into(),
            name: name.into(),
            category: "maths".into(),
            current_seq: 1,
            enabled: true,
            disabled_by: None,
            last_called_at: None,
            created_at: 0,
            updated_at: 0,
            current: CustomToolVersion {
                tool_id: format!("id-{name}"),
                seq: 1,
                definition: def,
                actor: CustomToolActor::Agent,
                trigger: VersionTrigger::Create,
                restored_from: None,
                reason: None,
                created_at: 0,
            },
        }
    }

    fn known() -> HashSet<String> {
        ["memory_search", "memory_read", "current_time"].map(String::from).into()
    }

    fn codes(checked: &Checked) -> Vec<&str> {
        checked.problems.iter().map(|p| p.code.as_str()).collect()
    }

    #[test]
    fn every_problem_is_reported_at_once() {
        let mut def = definition("Bad", "memory_serch(query=1");
        def.input_schema = json!({"type": "string"});
        let checked = validate_definition(&mut def, Some(&known()), &[], None);
        assert_eq!(codes(&checked), vec!["name_invalid", "schema_not_object", "script_syntax"]);

        let mut def = definition("bad", "memory_serch(query=1)");
        let checked = validate_definition(&mut def, Some(&known()), &[], None);
        assert_eq!(codes(&checked), vec!["unknown_tool"]);
        assert_eq!(checked.problems[0].extra["did_you_mean"], json!(["memory_search"]));
    }

    #[test]
    fn without_known_names_unknown_calls_are_a_warning() {
        let mut def = definition("ok", "stripe_list_charges()");
        let checked = validate_definition(&mut def, None, &[], None);
        assert!(checked.problems.is_empty());
        assert_eq!(checked.warnings[0].code, "tools_unchecked");
    }

    #[test]
    fn cycles_are_refused_with_their_path() {
        let tools = vec![saved("double_sum", "echo_sum(a=1, b=1) * 2"), saved("echo_sum", "1")];
        let mut def = definition("echo_sum", "double_sum(a=1, b=1)");
        let checked = validate_definition(&mut def, Some(&known()), &tools, Some("id-echo_sum"));
        assert_eq!(codes(&checked), vec!["cycle"]);
        assert_eq!(checked.problems[0].message, "cycle: echo_sum → double_sum → echo_sum");

        // Three tools.
        let tools = vec![saved("a", "b()"), saved("b", "c()"), saved("c", "1")];
        let mut def = definition("c", "a()");
        let checked = validate_definition(&mut def, Some(&known()), &tools, Some("id-c"));
        assert_eq!(checked.problems[0].message, "cycle: c → a → b → c");

        // Calling yourself is the shortest cycle.
        let mut def = definition("self_call", "self_call()");
        assert_eq!(codes(&validate_definition(&mut def, Some(&known()), &[], None)), vec!["cycle"]);
    }

    #[test]
    fn a_sample_input_must_match_the_schema() {
        let mut def = definition("s", "input['n'] * 2");
        def.input_schema = json!({"type": "object", "properties": {"n": {"type": "integer"}}});
        def.sample_input = Some(json!({"n": "3"}));
        assert_eq!(codes(&validate_definition(&mut def, Some(&known()), &[], None)), vec!["sample_invalid"]);
    }

    #[test]
    fn attention_and_model_backing_are_transitive() {
        let tools = vec![saved("inner", "stripe_list_charges()\nspawn_subagents()"), saved("outer", "inner()")];
        let names = known();
        assert_eq!(
            needs_attention(&["inner".into()], Some(&names), &tools).unwrap(),
            vec!["spawn_subagents".to_string(), "stripe_list_charges".to_string()]
        );
        let backed: HashSet<String> = ["spawn_subagents".to_string()].into();
        assert!(model_backed(&["inner".into()], &backed, &tools));
        assert!(!model_backed(&["memory_read".into()], &backed, &tools));
        assert_eq!(callers("inner", &tools), vec!["outer".to_string()]);
    }
}
