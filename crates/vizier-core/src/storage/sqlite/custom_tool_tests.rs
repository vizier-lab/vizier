//! The storage contract of `specs/061-agent-custom-tools/data-model.md` (task T015).

use std::sync::Arc;

use super::*;
use crate::storage::{document::BlobDocumentStore, sqlite::init_custom_tool_schema};

fn setup() -> (SqlStorage, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::tests_support::test_store(|| {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        init_custom_tool_schema(&conn).unwrap();
        crate::storage::sql::SqlStore::sqlite(conn)
    });
    let storage = SqlStorage::new(store, Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())));
    (storage, dir)
}

const AGENT: &str = "dp-a";

fn version(name: &str, category: &str, script: &str) -> NewVersion {
    NewVersion {
        definition: CustomToolDefinition {
            name: name.into(),
            category: category.into(),
            description: format!("{name} tool"),
            input_schema: serde_json::json!({ "type": "object" }),
            output_description: None,
            script: script.into(),
            calls: vec![],
            sample_input: None,
        },
        trigger: VersionTrigger::Create,
        restored_from: None,
        reason: None,
    }
}

fn alice() -> CustomToolActor {
    CustomToolActor::Person { user_id: "u1".into(), name: "alice".into() }
}

async fn create(storage: &SqlStorage, name: &str, category: &str) -> CustomTool {
    match storage
        .save_custom_tool_version(
            AGENT,
            None,
            None,
            version(name, category, "1"),
            &CustomToolActor::Agent,
            Some("A category"),
        )
        .await
        .unwrap()
    {
        SaveOutcome::Saved { tool, .. } => tool,
        other => panic!("expected a save, got {other:?}"),
    }
}

#[tokio::test]
async fn create_then_update_keeps_every_version() {
    let (storage, _dir) = setup();
    let tool = create(&storage, "echo", "maths").await;
    assert_eq!(tool.current_seq, 1);

    let mut next = version("echo", "maths", "2");
    next.trigger = VersionTrigger::Update;
    let SaveOutcome::Saved { tool: updated, category_created } = storage
        .save_custom_tool_version(AGENT, Some(&tool.id), Some(1), next, &CustomToolActor::Agent, None)
        .await
        .unwrap()
    else {
        panic!("update refused");
    };
    assert!(!category_created);
    assert_eq!(updated.current_seq, 2);
    assert_eq!(updated.current.definition.script, "2");
    let first = storage.get_custom_tool_version(AGENT, &tool.id, 1).await.unwrap().unwrap();
    assert_eq!(first.definition.script, "1");
    let versions = storage.list_custom_tool_versions(AGENT, &tool.id, None, 10).await.unwrap();
    assert_eq!(versions.iter().map(|v| v.seq).collect::<Vec<_>>(), vec![2, 1]);
}

#[tokio::test]
async fn a_stale_expected_version_is_out_of_date_and_writes_nothing() {
    let (storage, _dir) = setup();
    let tool = create(&storage, "echo", "maths").await;
    for _ in 0..2 {
        // Two savers that both read version 1: the second must not lose the first.
        let outcome = storage
            .save_custom_tool_version(
                AGENT,
                Some(&tool.id),
                Some(1),
                version("echo", "maths", "x"),
                &CustomToolActor::Agent,
                None,
            )
            .await
            .unwrap();
        let _ = outcome;
    }
    let current = storage.get_custom_tool(AGENT, "echo").await.unwrap().unwrap();
    assert_eq!(current.current_seq, 2);
    let outcome = storage
        .save_custom_tool_version(
            AGENT,
            Some(&tool.id),
            Some(1),
            version("echo", "maths", "y"),
            &CustomToolActor::Agent,
            None,
        )
        .await
        .unwrap();
    assert_eq!(outcome, SaveOutcome::OutOfDate { current_seq: 2 });
}

#[tokio::test]
async fn names_are_unique_per_agent_and_a_new_category_needs_a_description() {
    let (storage, _dir) = setup();
    create(&storage, "echo", "maths").await;
    let taken = storage
        .save_custom_tool_version(AGENT, None, None, version("echo", "maths", "1"), &CustomToolActor::Agent, None)
        .await
        .unwrap();
    assert_eq!(taken, SaveOutcome::NameTaken);
    let undescribed = storage
        .save_custom_tool_version(AGENT, None, None, version("other", "newcat", "1"), &CustomToolActor::Agent, None)
        .await
        .unwrap();
    assert_eq!(undescribed, SaveOutcome::CategoryDescriptionRequired);
    // Another agent may use the same name.
    let elsewhere = storage
        .save_custom_tool_version("dp-b", None, None, version("echo", "maths", "1"), &CustomToolActor::Agent, Some("x"))
        .await
        .unwrap();
    assert!(matches!(elsewhere, SaveOutcome::Saved { category_created: true, .. }));
}

#[tokio::test]
async fn moving_the_last_tool_away_deletes_its_category() {
    let (storage, _dir) = setup();
    let tool = create(&storage, "echo", "maths").await;
    create(&storage, "other", "reports").await;
    let mut moved = version("echo", "reports", "1");
    moved.trigger = VersionTrigger::Update;
    storage
        .save_custom_tool_version(AGENT, Some(&tool.id), None, moved, &CustomToolActor::Agent, None)
        .await
        .unwrap();
    let names: Vec<String> = storage
        .list_custom_tool_categories(AGENT)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(names, vec!["reports".to_string()]);
}

#[tokio::test]
async fn a_category_rename_keeps_ids_and_history_and_records_events() {
    let (storage, _dir) = setup();
    let tool = create(&storage, "echo", "maths").await;
    create(&storage, "taken", "other").await;
    assert_eq!(
        storage
            .update_custom_tool_category(AGENT, "maths", None, Some("other"), &alice())
            .await
            .unwrap(),
        CategoryOutcome::NameTaken
    );
    let CategoryOutcome::Updated { category, moved } = storage
        .update_custom_tool_category(AGENT, "maths", Some("Arithmetic"), Some("arith"), &alice())
        .await
        .unwrap()
    else {
        panic!("rename refused");
    };
    assert_eq!(category.name, "arith");
    assert_eq!(category.description, "Arithmetic");
    assert_eq!(moved, vec!["echo".to_string()]);
    let after = storage.get_custom_tool(AGENT, "echo").await.unwrap().unwrap();
    assert_eq!(after.id, tool.id);
    assert_eq!(after.category, "arith");
    assert_eq!(after.current_seq, 1, "a rename is not a version");
    let kinds: Vec<String> =
        storage.list_custom_tool_events(AGENT, None, 10).await.unwrap().into_iter().map(|e| e.kind).collect();
    assert!(kinds.contains(&"category_renamed".to_string()));
    assert!(kinds.contains(&"category_described".to_string()));
}

#[tokio::test]
async fn disable_and_delete_are_events_and_delete_removes_the_rows() {
    let (storage, _dir) = setup();
    let tool = create(&storage, "echo", "maths").await;
    let disabled = storage.set_custom_tool_enabled(AGENT, &tool.id, false, &alice()).await.unwrap().unwrap();
    assert!(!disabled.enabled);
    assert_eq!(disabled.disabled_by, Some(alice()));
    assert!(storage.list_custom_tools(AGENT, false).await.unwrap().is_empty());
    assert_eq!(storage.list_custom_tools(AGENT, true).await.unwrap().len(), 1);
    let category = &storage.list_custom_tool_categories(AGENT).await.unwrap()[0];
    assert_eq!((category.tools, category.enabled_tools), (1, 0));

    assert!(storage.delete_custom_tool(AGENT, &tool.id, &alice()).await.unwrap());
    assert!(storage.get_custom_tool(AGENT, "echo").await.unwrap().is_none());
    assert!(storage.get_custom_tool_version(AGENT, &tool.id, 1).await.unwrap().is_none());
    assert!(storage.list_custom_tool_categories(AGENT).await.unwrap().is_empty());
    let events = storage.list_custom_tool_events(AGENT, None, 10).await.unwrap();
    assert!(events.iter().any(|e| e.kind == "deleted" && e.tool_name.as_deref() == Some("echo")));
}

#[tokio::test]
async fn deleting_the_agent_removes_every_custom_tool_row() {
    let (storage, _dir) = setup();
    let tool = create(&storage, "echo", "maths").await;
    storage.set_custom_tool_enabled(AGENT, &tool.id, false, &alice()).await.unwrap();
    {
        let conn = storage.conn.lock().unwrap();
        delete_agent_custom_tools(&conn, AGENT).unwrap();
    }
    assert!(storage.list_custom_tools(AGENT, true).await.unwrap().is_empty());
    assert!(storage.list_custom_tool_categories(AGENT).await.unwrap().is_empty());
    assert!(storage.list_custom_tool_events(AGENT, None, 10).await.unwrap().is_empty());
    assert!(storage.get_custom_tool_version(AGENT, &tool.id, 1).await.unwrap().is_none());
}
