//! `UiPages` over a real SQLite store and a local blob store (task T014).

use std::sync::Arc;

use super::*;
use crate::{
    schema::VizierChannelId,
    storage::{
        blob::LocalBlobStore, document::BlobDocumentStore, sqlite::SqlStorage,
    },
};

struct Harness {
    pages: UiPages,
    blobs: Arc<dyn BlobStore>,
    _dir: tempfile::TempDir,
}

fn harness() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::tests_support::test_store(|| {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_ui_page_schema(&conn).unwrap();
        crate::storage::sql::SqlStore::sqlite(conn)
    });
    let storage = SqlStorage::new(store, Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())));
    let blobs: Arc<dyn BlobStore> = Arc::new(LocalBlobStore::new(dir.path()));
    Harness {
        pages: UiPages::new(Arc::new(VizierStorage::new(storage)), blobs.clone()),
        blobs,
        _dir: dir,
    }
}

fn session(topic: &str) -> VizierSession {
    VizierSession("dp".into(), VizierChannelId::HTTP("ana".into(), "webui".into()), Some(topic.into()))
}

#[test]
fn page_ids_are_prefixed_and_distinct() {
    let a = new_page_id();
    assert!(a.starts_with("pg_") && a.len() == 25, "{a}");
    assert_ne!(a, new_page_id());
}

#[tokio::test]
async fn create_then_add_versions_keeps_each_html() {
    let h = harness();
    let page = h.pages.create(&session("t"), "Chess", "<h1>v1</h1>").await.unwrap();
    assert_eq!(page.latest_version, 1);

    assert_eq!(
        h.pages.add_version(&page, 1, "<h1>v2</h1>", Made::Edits { count: 1 }, None).await.unwrap(),
        NewVersion::Added(2)
    );
    assert_eq!(
        h.pages.add_version(&page, 1, "<h1>late</h1>", Made::Full, None).await.unwrap(),
        NewVersion::Stale { latest: 2 }
    );
    assert_eq!(h.pages.html(&page, 1).await.unwrap().as_deref(), Some("<h1>v1</h1>"));
    assert_eq!(h.pages.html(&page, 2).await.unwrap().as_deref(), Some("<h1>v2</h1>"));

    let stored = h.pages.storage().list_ui_page_versions(&page.id).await.unwrap();
    assert_eq!(stored[0].sha256, digest("<h1>v1</h1>"));
    assert_eq!(stored[1].origin, VersionOrigin::Edits);
    assert_eq!(stored[1].edit_count, Some(1));
}

#[tokio::test]
async fn deleting_a_conversation_removes_rows_and_blobs() {
    let h = harness();
    let gone = h.pages.create(&session("t"), "A", "<p>a</p>").await.unwrap();
    let kept = h.pages.create(&session("u"), "B", "<p>b</p>").await.unwrap();

    h.pages.delete_session(&session("t")).await;
    assert!(h.pages.storage().get_ui_page(&gone.id).await.unwrap().is_none());
    let prefix = page_prefix("dp", &gone.id).unwrap();
    assert!(h.blobs.list(&prefix).await.unwrap().is_empty());
    assert_eq!(h.pages.html(&kept, 1).await.unwrap().as_deref(), Some("<p>b</p>"));

    h.pages.delete_agent_blobs("dp").await;
    assert_eq!(h.pages.html(&kept, 1).await.unwrap(), None);
}

#[tokio::test]
async fn a_ticket_opens_its_version_and_a_forged_one_nothing() {
    let h = harness();
    let page = h.pages.create(&session("t"), "A", "<p>a</p>").await.unwrap();
    let (ticket, _) = h.pages.mint_ticket(&page.id, 1);
    let (opened, version, html) = h.pages.open_ticket(&ticket).await.unwrap().unwrap();
    assert_eq!((opened.id.as_str(), version, html.as_str()), (page.id.as_str(), 1, "<p>a</p>"));

    let (missing, _) = h.pages.mint_ticket(&page.id, 9);
    assert!(h.pages.open_ticket(&missing).await.unwrap().is_none(), "no such version");
    assert!(h.pages.open_ticket(&format!("{ticket}x")).await.unwrap().is_none(), "forged");
}
