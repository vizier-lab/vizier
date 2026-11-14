//! Service-level tests of `specs/061-agent-custom-tools` (stories 1, 4, 6, 8).

use std::sync::Arc;

use serde_json::json;

use super::*;
use crate::storage::{document::BlobDocumentStore, sqlite::SqlStorage};

fn service() -> (CustomTools, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::tests_support::test_store(|| {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_custom_tool_schema(&conn).unwrap();
        crate::storage::sql::SqlStore::sqlite(conn)
    });
    let storage = Arc::new(VizierStorage::new(SqlStorage::new(
        store,
        Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
    )));
    (CustomTools::new(storage, None), dir)
}

fn env(actor: CustomToolActor) -> Env<'static> {
    Env {
        agent_id: "dp-a".into(),
        actor,
        names: Some(ToolNames {
            names: vec!["memory_search".into(), "memory_read".into(), "current_time".into()],
            model_backed: vec![],
        }),
        runner: None,
        reserved: vec!["brave_search".into()],
    }
}

fn agent() -> Env<'static> {
    env(CustomToolActor::Agent)
}

fn person() -> Env<'static> {
    env(CustomToolActor::Person { user_id: "u1".into(), name: "alice".into() })
}

fn create_input(name: &str, script: &str) -> CreateInput {
    CreateInput {
        name: name.into(),
        description: "Add two numbers".into(),
        input_schema: json!({"type": "object", "properties": {"a": {"type": "integer"}, "b": {"type": "integer"}}}),
        script: script.into(),
        category: Some("maths".into()),
        category_description: Some("Small arithmetic helpers".into()),
        output_description: None,
        sample_input: None,
        reason: None,
    }
}

fn update(name: &str) -> UpdateInput {
    UpdateInput { name: name.into(), ..Default::default() }
}

#[tokio::test]
async fn create_reports_call_as_and_category_and_refuses_duplicates() {
    let (tools, _dir) = service();
    let created = tools.create(&agent(), create_input("echo_sum", "input['a'] + input['b']")).await.unwrap();
    assert_eq!(created["status"], "created");
    assert_eq!(created["tool"]["call_as"], "echo_sum");
    assert_eq!(created["version"], 1);
    assert_eq!(created["category_created"], true);

    let again = tools.create(&agent(), create_input("echo_sum", "1")).await.unwrap();
    assert_eq!(again["status"], "invalid");
    assert_eq!(again["problems"][0]["code"], "name_taken");
}

#[tokio::test]
async fn a_new_category_needs_a_description_and_the_default_does_not() {
    let (tools, _dir) = service();
    let mut input = create_input("nocat", "1");
    input.category = Some("newcat".into());
    input.category_description = None;
    let refused = tools.create(&agent(), input).await.unwrap();
    assert_eq!(refused["problems"][0]["code"], "category_description_required");

    let mut input = create_input("loose", "42");
    input.category = None;
    input.category_description = None;
    let created = tools.create(&agent(), input).await.unwrap();
    assert_eq!(created["tool"]["category"], "custom");
}

#[tokio::test]
async fn a_clashing_category_is_renamed_and_reported() {
    let (tools, _dir) = service();
    let mut input = create_input("memory", "1");
    input.category = Some("memory".into());
    let created = tools.create(&agent(), input).await.unwrap();
    assert_eq!(created["category_renamed_to"], "memory_custom");
    let mut input = create_input("search", "1");
    input.category = Some("brave_search".into());
    let created = tools.create(&agent(), input).await.unwrap();
    assert_eq!(created["category_renamed_to"], "brave_search_custom");
}

#[tokio::test]
async fn versions_diff_and_rollback() {
    let (tools, _dir) = service();
    tools.create(&agent(), create_input("echo_sum", "input['a'] + input['b']")).await.unwrap();
    let v2 = tools
        .update(&agent(), UpdateInput { script: Some("input['a'] + input['b'] + 0".into()), ..update("echo_sum") })
        .await
        .unwrap();
    assert_eq!(v2["version"], 2);
    let v3 = tools
        .update(&agent(), UpdateInput { description: Some("Add two integers".into()), ..update("echo_sum") })
        .await
        .unwrap();
    assert_eq!(v3["version"], 3);
    let diff = tools.diff("dp-a", "echo_sum", 1, Some(3)).await.unwrap();
    assert!(diff["description"]["additions"].as_u64().unwrap() > 0);
    assert!(diff["script"]["additions"].as_u64().unwrap() > 0);

    let rolled = tools.rollback(&agent(), "echo_sum", 1, None, None).await.unwrap();
    assert_eq!(rolled["status"], "rolled_back");
    assert_eq!(rolled["version"], 4);
    assert_eq!(rolled["restored_from"], 1);

    // A broken change keeps the current version.
    let broken = tools
        .update(&agent(), UpdateInput { script: Some("input[".into()), expected_version: Some(4), ..update("echo_sum") })
        .await
        .unwrap();
    assert_eq!(broken["status"], "invalid");
    let current = tools.get("dp-a", None, "echo_sum", None).await.unwrap();
    assert_eq!(current["version"], 4);

    let stale = tools
        .update(&agent(), UpdateInput { script: Some("1".into()), expected_version: Some(2), ..update("echo_sum") })
        .await
        .unwrap();
    assert_eq!(stale["status"], "out_of_date");
    assert_eq!(stale["current_version"], 4);

    let history = tools.history("dp-a", "echo_sum").await.unwrap();
    assert_eq!(history["versions"].as_array().unwrap().len(), 4);
}

#[tokio::test]
async fn composition_cycles_and_callers() {
    let (tools, _dir) = service();
    tools.create(&agent(), create_input("echo_sum", "input['a'] + input['b']")).await.unwrap();
    let double = tools
        .create(&agent(), create_input("double_sum", "echo_sum(a=input['a'], b=input['b']) * 2"))
        .await
        .unwrap();
    assert_eq!(double["status"], "created");

    let cycle = tools
        .update(&agent(), UpdateInput { script: Some("double_sum(a=1, b=1)".into()), ..update("echo_sum") })
        .await
        .unwrap();
    assert_eq!(cycle["problems"][0]["code"], "cycle");
    assert_eq!(cycle["problems"][0]["message"], "cycle: echo_sum → double_sum → echo_sum");

    let rename = tools
        .update(&agent(), UpdateInput { new_name: Some("add".into()), ..update("echo_sum") })
        .await
        .unwrap();
    assert_eq!(rename["status"], "has_callers");
    assert_eq!(rename["callers"], json!(["double_sum"]));
    assert_eq!(tools.delete(&agent(), "echo_sum").await.unwrap()["status"], "has_callers");
    // A person may, after confirming.
    assert_eq!(tools.delete(&person(), "echo_sum").await.unwrap()["status"], "deleted");
    let names = agent().names;
    let listed = tools.list("dp-a", names.as_ref(), None, true).await.unwrap();
    let double = listed["tools"].as_array().unwrap().iter().find(|t| t["name"] == "double_sum").unwrap();
    assert_eq!(double["needs_attention"]["missing"], json!(["echo_sum"]));
}

#[tokio::test]
async fn a_person_switch_off_holds_against_the_agent() {
    let (tools, _dir) = service();
    tools.create(&agent(), create_input("echo_sum", "1")).await.unwrap();
    assert_eq!(tools.set_enabled(&person(), "echo_sum", false).await.unwrap()["status"], "disabled");
    assert_eq!(tools.set_enabled(&agent(), "echo_sum", true).await.unwrap()["status"], "disabled_by_person");
    let changed = tools
        .update(&agent(), UpdateInput { script: Some("2".into()), ..update("echo_sum") })
        .await
        .unwrap();
    assert_eq!(changed["status"], "disabled_by_person");
    assert_eq!(tools.set_enabled(&person(), "echo_sum", true).await.unwrap()["status"], "enabled");
}

#[tokio::test]
async fn categories_rename_move_and_clean_up() {
    let (tools, _dir) = service();
    tools.create(&agent(), create_input("echo_sum", "1")).await.unwrap();
    let mut loose = create_input("loose", "1");
    loose.category = None;
    tools.create(&agent(), loose).await.unwrap();

    let renamed = tools.update_category(&agent(), "maths", Some("Arithmetic"), Some("arith")).await.unwrap();
    assert_eq!(renamed["renamed_to"], "arith");
    let tool = tools.get("dp-a", None, "echo_sum", None).await.unwrap();
    assert_eq!(tool["category"], "arith");
    assert_eq!(tool["call_as"], "echo_sum");

    let moved = tools
        .update(&agent(), UpdateInput { category: Some("arith".into()), ..update("loose") })
        .await
        .unwrap();
    assert_eq!(moved["version"], 2, "a move is a version");
    let listed = tools.list("dp-a", None, None, true).await.unwrap();
    let names: Vec<&str> =
        listed["categories"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["arith"], "the emptied default category is gone");

    // Switched off: the row stays, the prompt line goes.
    tools.set_enabled(&agent(), "echo_sum", false).await.unwrap();
    tools.set_enabled(&agent(), "loose", false).await.unwrap();
    let router = crate::agents::tools::ToolRouter::empty();
    assert!(tools.turn_categories("dp-a", &router).await.is_empty());
    assert_eq!(tools.list("dp-a", None, None, true).await.unwrap()["categories"].as_array().unwrap().len(), 1);
}

/// No prefix (FR-011): a name any tool of the agent has, a builtin, a sandbox tool or a retired
/// name is refused on save.
#[tokio::test]
async fn a_name_another_tool_has_is_refused_on_save() {
    let (tools, _dir) = service();
    for name in ["current_time", "len", "execute_python", "parallel_subagents"] {
        let refused = tools.create(&agent(), create_input(name, "1")).await.unwrap();
        assert_eq!(refused["status"], "invalid", "{name}: {refused}");
        assert_eq!(refused["problems"][0]["code"], "name_taken", "{name}: {refused}");
    }
    // A rename onto one is refused too.
    tools.create(&agent(), create_input("echo_sum", "1")).await.unwrap();
    let rename = tools
        .update(&agent(), UpdateInput { new_name: Some("memory_read".into()), ..update("echo_sum") })
        .await
        .unwrap();
    assert_eq!(rename["problems"][0]["code"], "name_taken", "{rename}");
}
