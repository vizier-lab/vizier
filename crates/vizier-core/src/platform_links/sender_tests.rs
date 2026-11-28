use super::*;
use crate::{
    platform_links::code,
    schema::{ChannelPlatform, VizierRequestContent},
    storage::{
        platform_link::PlatformLinkStorage, sql::SqlStore,
        sqlite::SqlStorage, user::UserStorage,
    },
};

const NOW: i64 = 1_800_000_000_000;

fn setup() -> (SqlStorage, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::tests_support::test_store(|| {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        SqlStorage::init_schema(&conn).unwrap();
        SqlStore::sqlite(conn)
    });
    let storage = SqlStorage::new(
        store,
    );
    (storage, dir)
}

fn person(id: &str, handle: &str) -> PlatformPerson {
    PlatformPerson {
        platform: ChannelPlatform::Discord,
        id: id.into(),
        username: Some(handle.into()),
        display_name: Some("Rin".into()),
    }
}

async fn link_rin(storage: &SqlStorage) -> String {
    let rin = storage.create_user("rin", "hash", "role").await.unwrap().user_id;
    storage
        .issue_connect_code(&person("1", "rin_dev"), &code::hash("AAAAAAAA"), NOW, NOW + 1000)
        .await
        .unwrap();
    storage
        .redeem_connect_code(&code::hash("AAAAAAAA"), &rin, NOW)
        .await
        .unwrap();
    rin
}

#[tokio::test]
async fn a_linked_account_reads_as_the_vizier_username() {
    let (storage, _dir) = setup();
    link_rin(&storage).await;

    let sender = resolve(&storage, person("1", "rin_dev")).await;
    assert_eq!(sender.label, "rin");
    assert!(sender.verified);
}

#[tokio::test]
async fn an_unlinked_account_is_unregistered_and_keeps_its_own_identity() {
    let (storage, _dir) = setup();
    link_rin(&storage).await;

    let kai = resolve(&storage, person("2", "kai.k")).await;
    let sam = resolve(&storage, person("3", "sam")).await;
    for sender in [&kai, &sam] {
        assert_eq!(sender.label, UNREGISTERED_USER);
        assert!(!sender.verified);
    }
    assert_ne!(kai.person.key(), sam.person.key());
    // The same id on Telegram is not the linked Discord account.
    let mut telegram = person("1", "rin_dev");
    telegram.platform = ChannelPlatform::Telegram;
    assert!(!resolve(&storage, telegram).await.verified);
}

#[tokio::test]
async fn unlinking_applies_to_the_next_message() {
    let (storage, _dir) = setup();
    let rin = link_rin(&storage).await;
    assert!(resolve(&storage, person("1", "rin_dev")).await.verified);

    storage.unlink_platform(&rin, ChannelPlatform::Discord).await.unwrap();

    assert_eq!(
        resolve(&storage, person("1", "rin_dev")).await.label,
        UNREGISTERED_USER
    );
}

#[tokio::test]
async fn a_changed_handle_is_remembered() {
    let (storage, _dir) = setup();
    link_rin(&storage).await;

    resolve(&storage, person("1", "rin_new")).await;

    let link = storage
        .find_link(ChannelPlatform::Discord, "1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(link.platform_username.as_deref(), Some("rin_new"));
}

#[tokio::test]
async fn stamping_sets_the_sender_and_keeps_the_readers_own_metadata() {
    let (storage, _dir) = setup();
    link_rin(&storage).await;
    let mut request = VizierRequest {
        user: "someone".into(),
        content: VizierRequestContent::Chat("hi".into()),
        metadata: serde_json::json!({ "discord_channel_id": "1100", "is_dm": false }),
        ..Default::default()
    };

    stamp(&mut request, &resolve(&storage, person("1", "rin_dev")).await);

    assert_eq!(request.user, "rin");
    assert_eq!(request.verified, Some(true));
    assert_eq!(
        request.metadata,
        serde_json::json!({
            "discord_channel_id": "1100",
            "is_dm": false,
            "platform": "discord",
            "platform_user_id": "1",
            "platform_username": "rin_dev",
            "platform_display_name": "Rin",
        })
    );
    let frontmatter = request.generate_frontmatter().unwrap();
    assert!(frontmatter.contains("sender: rin"), "{frontmatter}");
    assert!(frontmatter.contains("verified: true"), "{frontmatter}");
}

#[tokio::test]
async fn an_account_without_a_handle_carries_none() {
    let (storage, _dir) = setup();
    let mut bare = person("5", "x");
    bare.username = None;
    let mut request = VizierRequest::default();

    stamp(&mut request, &resolve(&storage, bare).await);

    assert!(request.metadata.get("platform_username").is_none());
    assert_eq!(request.metadata["platform_user_id"], "5");
    let frontmatter = request.generate_frontmatter().unwrap();
    assert!(frontmatter.contains("sender: Unregistered User"), "{frontmatter}");
    assert!(frontmatter.contains("verified: false"), "{frontmatter}");
}
