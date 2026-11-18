//! The storage contract of `specs/063-html-generative-ui/data-model.md` (tasks T009, T011).

use std::sync::Arc;

use super::*;
use crate::storage::{document::BlobDocumentStore, sqlite::init_ui_page_schema};

fn setup() -> (SqlStorage, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::tests_support::test_store(|| {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        init_ui_page_schema(&conn).unwrap();
        crate::storage::sql::SqlStore::sqlite(conn)
    });
    let storage = SqlStorage::new(store, Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())));
    (storage, dir)
}

fn session(topic: &str) -> VizierSession {
    VizierSession(
        "dp".into(),
        VizierChannelId::HTTP("ana".into(), "webui".into()),
        Some(topic.into()),
    )
}

fn page(id: &str, topic: &str) -> UiPageRecord {
    UiPageRecord {
        id: id.into(),
        agent_id: "dp".into(),
        session: session(topic),
        title: "Chess".into(),
        latest_version: 1,
        state: None,
        state_seq: 0,
        created_at: 1_000,
        updated_at: 1_000,
    }
}

fn full(size: u64) -> NewUiPageVersion {
    NewUiPageVersion {
        size,
        sha256: format!("{size:064x}"),
        origin: VersionOrigin::Full,
        base_version: None,
        edit_count: None,
    }
}

fn edits(base: u32) -> NewUiPageVersion {
    NewUiPageVersion {
        size: 10,
        sha256: "ab".repeat(32),
        origin: VersionOrigin::Edits,
        base_version: Some(base),
        edit_count: Some(2),
    }
}

#[tokio::test]
async fn create_reads_back_page_and_first_version() {
    let (s, _dir) = setup();
    s.create_ui_page(&page("pg_1", "t"), &full(42)).await.unwrap();

    let read = s.get_ui_page("pg_1").await.unwrap().unwrap();
    assert_eq!(read.session, session("t"));
    assert_eq!(read.latest_version, 1);
    assert_eq!(read.state, None);

    let v1 = s.get_ui_page_version("pg_1", 1).await.unwrap().unwrap();
    assert_eq!(v1.size, 42);
    assert_eq!(v1.origin, VersionOrigin::Full);
    assert_eq!(v1.verification, pending_verification());
    assert!(s.get_ui_page("nope").await.unwrap().is_none());
}

#[tokio::test]
async fn versions_advance_only_from_the_latest() {
    let (s, _dir) = setup();
    s.create_ui_page(&page("pg_1", "t"), &full(1)).await.unwrap();

    assert_eq!(
        s.add_ui_page_version("pg_1", 1, &edits(1), None).await.unwrap(),
        AddVersionOutcome::Added(2)
    );
    // A second edit set based on the same version loses (spec edge case "two edit sets").
    assert_eq!(
        s.add_ui_page_version("pg_1", 1, &edits(1), None).await.unwrap(),
        AddVersionOutcome::Stale { latest: 2 }
    );
    assert_eq!(
        s.add_ui_page_version("pg_1", 2, &full(5), Some("Chess (wooden)")).await.unwrap(),
        AddVersionOutcome::Added(3)
    );
    assert_eq!(
        s.add_ui_page_version("missing", 1, &full(5), None).await.unwrap(),
        AddVersionOutcome::NotFound
    );

    let page = s.get_ui_page("pg_1").await.unwrap().unwrap();
    assert_eq!(page.latest_version, 3);
    assert_eq!(page.title, "Chess (wooden)");
    let versions = s.list_ui_page_versions("pg_1").await.unwrap();
    assert_eq!(versions.iter().map(|v| v.version).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(versions[1].base_version, Some(1));
    assert_eq!(versions[1].edit_count, Some(2));
}

#[tokio::test]
async fn verification_and_history_uid_are_recorded() {
    let (s, _dir) = setup();
    s.create_ui_page(&page("pg_1", "t"), &full(1)).await.unwrap();
    let clean = serde_json::json!({"state": "clean", "observed_ms": 2000});
    s.set_ui_page_verification("pg_1", 1, &clean).await.unwrap();
    s.set_ui_page_history_uid("pg_1", 1, "h_1").await.unwrap();

    let v1 = s.get_ui_page_version("pg_1", 1).await.unwrap().unwrap();
    assert_eq!(v1.verification, clean);
    assert_eq!(v1.history_uid.as_deref(), Some("h_1"));
}

#[tokio::test]
async fn state_is_saved_with_a_sequence() {
    let (s, _dir) = setup();
    s.create_ui_page(&page("pg_1", "t"), &full(1)).await.unwrap();
    let state = serde_json::json!({"fen": "start"});

    assert_eq!(s.save_ui_page_state("pg_1", &state, None).await.unwrap(), SaveStateOutcome::Saved(1));
    assert_eq!(s.save_ui_page_state("pg_1", &state, Some(0)).await.unwrap(), SaveStateOutcome::Stale(1));
    assert_eq!(s.save_ui_page_state("pg_1", &state, Some(1)).await.unwrap(), SaveStateOutcome::Saved(2));
    assert_eq!(s.save_ui_page_state("nope", &state, None).await.unwrap(), SaveStateOutcome::NotFound);

    let page = s.get_ui_page("pg_1").await.unwrap().unwrap();
    assert_eq!(page.state, Some(state));
    assert_eq!(page.state_seq, 2);
}

#[tokio::test]
async fn deleting_a_conversation_or_agent_deletes_its_pages() {
    let (s, _dir) = setup();
    s.create_ui_page(&page("pg_1", "t"), &full(1)).await.unwrap();
    s.add_ui_page_version("pg_1", 1, &edits(1), None).await.unwrap();
    s.create_ui_page(&page("pg_2", "other"), &full(1)).await.unwrap();

    assert_eq!(s.delete_ui_pages_for_session(&session("t")).await.unwrap(), vec!["pg_1".to_string()]);
    assert!(s.get_ui_page("pg_1").await.unwrap().is_none());
    assert!(s.list_ui_page_versions("pg_1").await.unwrap().is_empty());
    assert!(s.get_ui_page("pg_2").await.unwrap().is_some());

    assert_eq!(s.delete_ui_pages_for_agent("dp").await.unwrap(), vec!["pg_2".to_string()]);
    assert!(s.get_ui_page("pg_2").await.unwrap().is_none());
}

#[tokio::test]
async fn sweep_closes_pending_verifications() {
    let (s, _dir) = setup();
    s.create_ui_page(&page("pg_1", "t"), &full(1)).await.unwrap();
    s.add_ui_page_version("pg_1", 1, &edits(1), None).await.unwrap();
    s.set_ui_page_verification("pg_1", 1, &serde_json::json!({"state": "clean"})).await.unwrap();

    // No fence held in this store: every row was written with no owner token.
    assert_eq!(s.sweep_pending_ui_verifications("dp", 1).await.unwrap(), 1);
    let v2 = s.get_ui_page_version("pg_1", 2).await.unwrap().unwrap();
    assert_eq!(v2.verification, serde_json::json!({"state": "unverified", "reason": "closed"}));
    let v1 = s.get_ui_page_version("pg_1", 1).await.unwrap().unwrap();
    assert_eq!(v1.verification, serde_json::json!({"state": "clean"}));
}
