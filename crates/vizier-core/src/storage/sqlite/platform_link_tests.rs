//! The storage rules of spec 069: single use, one owner per account, one account per platform.

use std::sync::Arc;

use super::*;
use crate::{
    platform_links::code,
    storage::{
        document::BlobDocumentStore, platform_link::PlatformLinkStorage, sql::SqlStore,
        user::UserStorage,
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

async fn user(storage: &SqlStorage, username: &str) -> String {
    storage
        .create_user(username, "hash", "role")
        .await
        .unwrap()
        .user_id
}

fn discord(id: &str) -> PlatformPerson {
    PlatformPerson {
        platform: ChannelPlatform::Discord,
        id: id.into(),
        username: Some(format!("handle_{id}")),
        display_name: Some(format!("Name {id}")),
    }
}

fn telegram(id: &str) -> PlatformPerson {
    PlatformPerson {
        platform: ChannelPlatform::Telegram,
        id: id.into(),
        username: None,
        display_name: Some(format!("Name {id}")),
    }
}

/// Issue `code` to `person`, live for ten minutes from [`NOW`].
async fn issue(storage: &SqlStorage, person: &PlatformPerson, code: &str) -> String {
    let hash = code::hash(code);
    storage
        .issue_connect_code(person, &hash, NOW, NOW + code::LIFETIME_MS)
        .await
        .unwrap();
    hash
}

async fn link(storage: &SqlStorage, person: &PlatformPerson, code: &str, user_id: &str) -> RedeemOutcome {
    let hash = issue(storage, person, code).await;
    storage.redeem_connect_code(&hash, user_id, NOW).await.unwrap()
}

#[tokio::test]
async fn a_code_works_once_and_links_the_account_it_was_issued_to() {
    let (storage, _dir) = setup();
    let rin = user(&storage, "rin").await;
    let hash = issue(&storage, &discord("1"), "AAAAAAAA").await;

    // Peeking consumes nothing.
    for _ in 0..2 {
        assert_eq!(
            storage.peek_connect_code(&hash, NOW).await.unwrap(),
            Some(discord("1"))
        );
    }

    let RedeemOutcome::Linked(linked) =
        storage.redeem_connect_code(&hash, &rin, NOW).await.unwrap()
    else {
        panic!("expected a link");
    };
    assert_eq!(linked.username, "rin");
    assert_eq!(linked.platform_user_id, "1");
    assert_eq!(linked.platform_username.as_deref(), Some("handle_1"));

    assert_eq!(
        storage.redeem_connect_code(&hash, &rin, NOW).await.unwrap(),
        RedeemOutcome::Unavailable
    );
    assert_eq!(storage.peek_connect_code(&hash, NOW).await.unwrap(), None);
    let found = storage
        .find_link(ChannelPlatform::Discord, "1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.user_id, rin);
    // The same id on another platform is another account.
    assert!(storage.find_link(ChannelPlatform::Telegram, "1").await.unwrap().is_none());
}

#[tokio::test]
async fn a_second_code_voids_the_first_and_an_expired_one_does_nothing() {
    let (storage, _dir) = setup();
    let rin = user(&storage, "rin").await;
    let first = issue(&storage, &discord("1"), "AAAAAAAA").await;
    let second = issue(&storage, &discord("1"), "BBBBBBBB").await;
    assert_eq!(
        storage.redeem_connect_code(&first, &rin, NOW).await.unwrap(),
        RedeemOutcome::Unavailable
    );

    let expired_at = NOW + code::LIFETIME_MS;
    assert_eq!(storage.peek_connect_code(&second, expired_at).await.unwrap(), None);
    assert_eq!(
        storage.redeem_connect_code(&second, &rin, expired_at).await.unwrap(),
        RedeemOutcome::Unavailable
    );
    assert!(storage.list_links(&rin).await.unwrap().is_empty());
    assert_eq!(storage.purge_expired_connect_codes(expired_at).await.unwrap(), 1);
    assert_eq!(storage.purge_expired_connect_codes(expired_at).await.unwrap(), 0);
}

#[tokio::test]
async fn an_account_linked_to_someone_else_is_refused_and_untouched() {
    let (storage, _dir) = setup();
    let rin = user(&storage, "rin").await;
    let kai = user(&storage, "kai").await;
    link(&storage, &discord("1"), "AAAAAAAA", &rin).await;

    let hash = issue(&storage, &discord("1"), "BBBBBBBB").await;
    assert_eq!(
        storage.redeem_connect_code(&hash, &kai, NOW).await.unwrap(),
        RedeemOutcome::LinkedElsewhere
    );
    assert_eq!(
        storage
            .find_link(ChannelPlatform::Discord, "1")
            .await
            .unwrap()
            .unwrap()
            .user_id,
        rin
    );
    assert!(storage.list_links(&kai).await.unwrap().is_empty());
    // Refusing leaves the code for its owner.
    assert!(storage.peek_connect_code(&hash, NOW).await.unwrap().is_some());
}

#[tokio::test]
async fn a_person_has_one_account_per_platform() {
    let (storage, _dir) = setup();
    let rin = user(&storage, "rin").await;
    link(&storage, &discord("1"), "AAAAAAAA", &rin).await;
    link(&storage, &telegram("9"), "BBBBBBBB", &rin).await;

    // Linking the same account again is not a replacement.
    assert!(matches!(
        link(&storage, &discord("1"), "CCCCCCCC", &rin).await,
        RedeemOutcome::Linked(_)
    ));

    let RedeemOutcome::Replaced { link: new, previous } =
        link(&storage, &discord("2"), "DDDDDDDD", &rin).await
    else {
        panic!("expected a replacement");
    };
    assert_eq!(new.platform_user_id, "2");
    assert_eq!(previous.platform_user_id, "1");
    assert!(storage.find_link(ChannelPlatform::Discord, "1").await.unwrap().is_none());
    let accounts: Vec<_> = storage
        .list_links(&rin)
        .await
        .unwrap()
        .into_iter()
        .map(|l| (l.platform, l.platform_user_id))
        .collect();
    assert_eq!(
        accounts,
        [
            (ChannelPlatform::Discord, "2".to_string()),
            (ChannelPlatform::Telegram, "9".to_string())
        ]
    );
    let actions: Vec<_> = storage
        .list_link_events(&rin, 20)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.action)
        .collect();
    assert_eq!(actions.len(), 4);
    assert!(actions.contains(&LinkAction::Replaced));
}

#[tokio::test]
async fn unlinking_touches_only_the_persons_own_link() {
    let (storage, _dir) = setup();
    let rin = user(&storage, "rin").await;
    let kai = user(&storage, "kai").await;
    link(&storage, &discord("1"), "AAAAAAAA", &rin).await;

    assert!(!storage.unlink_platform(&kai, ChannelPlatform::Discord).await.unwrap());
    assert!(!storage.unlink_platform(&rin, ChannelPlatform::Telegram).await.unwrap());
    assert!(storage.unlink_platform(&rin, ChannelPlatform::Discord).await.unwrap());
    assert!(!storage.unlink_platform(&rin, ChannelPlatform::Discord).await.unwrap());
    assert!(storage.find_link(ChannelPlatform::Discord, "1").await.unwrap().is_none());
    let events = storage.list_link_events(&rin, 20).await.unwrap();
    assert!(events.iter().any(|e| e.action == LinkAction::Unlinked));
}

#[tokio::test]
async fn linking_forgets_what_was_typed_by_hand_for_that_platform_only() {
    let (storage, _dir) = setup();
    let rin = user(&storage, "rin").await;
    let typed = UserProfile {
        user_id: rin.clone(),
        discord_id: Some("1".into()),
        discord_username: Some("old".into()),
        telegram_id: Some("9".into()),
        telegram_username: Some("old_t".into()),
        alias: vec!["Rin".into()],
    };
    storage.upsert_user_profile(&rin, &typed).await.unwrap();

    link(&storage, &discord("1"), "AAAAAAAA", &rin).await;

    let profile = storage.get_user_profile(&rin).await.unwrap().unwrap();
    assert_eq!(profile.discord_id, None);
    assert_eq!(profile.discord_username, None);
    assert_eq!(profile.telegram_id.as_deref(), Some("9"));
    assert_eq!(profile.telegram_username.as_deref(), Some("old_t"));
    assert_eq!(profile.alias, ["Rin"]);
}

#[tokio::test]
async fn deleting_a_person_removes_their_links_and_history() {
    let (storage, _dir) = setup();
    let rin = user(&storage, "rin").await;
    link(&storage, &discord("1"), "AAAAAAAA", &rin).await;

    storage.delete_user(&rin).await.unwrap();

    assert!(storage.find_link(ChannelPlatform::Discord, "1").await.unwrap().is_none());
    assert!(storage.list_link_events(&rin, 20).await.unwrap().is_empty());
}
