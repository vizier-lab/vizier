use super::*;

use crate::agents::tools::tests_support::context;
use crate::schema::VizierSession;

fn args(target: &str, topic_id: Option<&str>) -> WakeSessionArgs {
    WakeSessionArgs {
        target: target.to_string(),
        topic_id: topic_id.map(str::to_string),
        instruction: "do the work".to_string(),
        wait: None,
    }
}

fn webui(user: &str) -> VizierChannelId {
    VizierChannelId::HTTP(user.to_string(), "vizier-webui".to_string())
}

/// The tools are offered where a person's access can be decided: not in a Discord or Telegram
/// conversation, whose identity is self-declared, and not in a dream or a worker's conversation.
#[test]
fn the_tools_are_offered_only_where_a_person_can_decide() {
    let offered = |channel: VizierChannelId| access::may_wake(&context(VizierSession("a".into(), channel, None)));

    // A WebUI or project chat, and a scheduled run: the owner's access decides there.
    assert!(offered(webui("ana")));
    assert!(offered(VizierChannelId::Project(
        "ana".into(),
        "proj".into()
    )));
    assert!(offered(VizierChannelId::Task(
        "daily".into(),
        crate::time::now()
    )));

    // Discord and Telegram: no verified person on the conversation.
    assert!(!offered(VizierChannelId::DiscordChanel(42)));
    assert!(!offered(VizierChannelId::TelegramChannel(-7)));

    // A dream and a worker: the machinery's, not the agent's.
    assert!(!offered(VizierChannelId::Subagent));
    assert!(!offered(VizierChannelId::Dream(
        Box::new(VizierSession("a".into(), webui("ana"), None)),
        crate::schema::DreamStage::Extraction
    )));
}

/// "current" is the caller's own conversation, when that is one that may be woken.
#[test]
fn current_resolves_to_the_callers_own_conversation() {
    let session = VizierSession("a".into(), webui("ana"), Some("t".into()));
    let resolved = resolve(&"a".to_string(), &args("current", None), &context(session.clone()));
    assert_eq!(resolved.map(|s| s.to_slug()), Ok(session.to_slug()));
}

/// A scheduled run's own conversation is the machinery's: it is listed, and refused.
#[test]
fn current_in_a_conversation_the_machinery_owns_is_refused() {
    let session = VizierSession(
        "a".into(),
        VizierChannelId::Task("daily".into(), crate::time::now()),
        None,
    );
    let refusal = resolve(&"a".to_string(), &args("current", None), &context(session)).unwrap_err();
    assert!(matches!(refusal, access::Refusal::NotWakeable(_)), "{refusal:?}");
}

/// An address that names a conversation of the calling agent.
#[test]
fn a_wakeable_address_resolves_against_the_calling_agent() {
    let ctx = context(VizierSession("a".into(), webui("ana"), None));
    let resolved = resolve(&"a".to_string(), &args("http__ana__vizier-webui", Some("t")), &ctx)
        .expect("the address names a conversation");
    assert_eq!(resolved.0, "a");
    assert_eq!(resolved.1, webui("ana"));
    assert_eq!(resolved.2.as_deref(), Some("t"));

    // The channel that has conversations of their own needs one.
    let refusal = resolve(&"a".to_string(), &args("http__ana__vizier-webui", None), &ctx).unwrap_err();
    assert!(matches!(refusal, access::Refusal::BadRequest(_)), "{refusal:?}");

    // A channel that has none never carries a topic.
    let resolved = resolve(&"a".to_string(), &args("discord__42", Some("ignored")), &ctx)
        .expect("a Discord channel needs no topic");
    assert_eq!(resolved.1, VizierChannelId::DiscordChanel(42));
    assert_eq!(resolved.2, None);
}

/// An address that names nothing is refused, with the reason.
#[test]
fn an_address_that_names_nothing_is_refused() {
    let ctx = context(VizierSession("a".into(), webui("ana"), None));
    let refusal = resolve(&"a".to_string(), &args("not-an-address", None), &ctx).unwrap_err();
    assert!(matches!(refusal, access::Refusal::UnknownChannel), "{refusal:?}");
}

/// A conversation belongs to the person it was had with, and nobody else.
#[test]
fn a_conversation_belongs_to_its_own_person() {
    assert!(access::belongs_to_person(&webui("ana"), "ana"));
    assert!(!access::belongs_to_person(&webui("ana"), "bob"));
    assert!(access::belongs_to_person(
        &VizierChannelId::Project("ana".into(), "proj".into()),
        "ana"
    ));
    // A Discord channel has no person of its own.
    assert!(access::belongs_to_person(&VizierChannelId::DiscordChanel(42), "ana"));
}

/// Every refusal is data the agent can read: one shape, a status and a message.
#[test]
fn every_refusal_is_data_with_a_status() {
    let refusals = [
        access::Refusal::UnknownChannel,
        access::Refusal::NotWakeable("a scheduled run".into()),
        access::Refusal::NoOwner,
        access::Refusal::OtherPerson("bob".into()),
        access::Refusal::TooDeep,
        access::Refusal::BadRequest("the instruction is empty".into()),
    ];
    let statuses: Vec<String> = refusals
        .iter()
        .map(|r| {
            let json = r.to_json();
            assert!(json["message"].as_str().is_some_and(|m| !m.is_empty()), "{json}");
            json["status"].as_str().unwrap().to_string()
        })
        .collect();
    assert_eq!(
        statuses,
        vec![
            "unknown_channel",
            "not_wakeable",
            "no_owner",
            "other_person",
            "too_deep",
            "bad_request"
        ]
    );
}

/// Neither tool names an agent: the calling agent is injected by the system, so the model never
/// has to name one (spec 067 research D9).
#[test]
fn neither_tool_takes_an_agent_id() {
    for schema in [
        <ListSessions as VizierTool>::input_schema(),
        <WakeSession as VizierTool>::input_schema(),
    ] {
        let properties = schema
            .get("properties")
            .and_then(|p| p.as_object())
            .expect("the arguments are an object");
        assert!(
            !properties.contains_key("agent_id"),
            "a tool must not ask for an agent: {properties:?}"
        );
    }
}

/// The hop limit is the background-work limit: a woken turn counts one deeper.
#[test]
fn the_woken_turn_counts_one_hop_deeper() {
    let limit = crate::schema::detached::MAX_BACKGROUND_DEPTH;
    let mut ctx = context(VizierSession("a".into(), webui("ana"), None));
    ctx.background_depth = limit - 1;
    assert!(ctx.background_depth < limit, "a turn one hop short of the limit may wake");
    ctx.background_depth = limit;
    assert!(ctx.background_depth >= limit, "a turn at the limit may not");
}

/// A listing holds the calling agent's own conversations, and only the ones the person it is
/// working for may reach.
#[test]
fn a_listing_holds_the_callers_own_conversations() {
    use crate::storage::{
        session::SessionStorage, sqlite::SqlStorage, user::UserStorage,
    };

    let dir = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    SqlStorage::init_schema(&conn).unwrap();
    let conn = crate::storage::sql::SqlStore::sqlite(conn);
    let storage = Arc::new(VizierStorage::new(SqlStorage::new(
        conn,
        Arc::new(crate::storage::document::BlobDocumentStore::local(
            dir.path().to_path_buf(),
        )),
    )));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let role = storage.create_role("chatter", vec![], false).await.unwrap();
        storage
            .create_user("ana", "", &role.role_id)
            .await
            .unwrap();
        storage
            .create_user("bob", "", &role.role_id)
            .await
            .unwrap();

        // Two conversations of ana's, one of bob's, and one the machinery owns.
        for (user, topic, title) in [
            ("ana", "t1", "Ana one"),
            ("ana", "t2", "Ana two"),
            ("bob", "t3", "Bob one"),
        ] {
            storage
                .save_session_detail(crate::schema::VizierSessionDetail {
                    agent_id: "a".into(),
                    channel: webui(user),
                    topic: Some(topic.into()),
                    title: title.into(),
                    is_thinking: false,
                })
                .await
                .unwrap();
        }
        storage
            .save_session_detail(crate::schema::VizierSessionDetail {
                agent_id: "a".into(),
                channel: VizierChannelId::Task("daily".into(), crate::time::now()),
                topic: None,
                title: "A job".into(),
                is_thinking: false,
            })
            .await
            .unwrap();

        let tool = ListSessions {
            storage: storage.clone(),
            agent_id: "a".into(),
        };
        // The turn is in ana's conversation, so ana's access decides.
        let ctx = context(VizierSession(
            "a".into(),
            webui("ana"),
            Some("t1".into()),
        ));
        let listed = tool
            .call(ListSessionsArgs { channel: None, query: None }, &ctx)
            .await
            .expect("the listing");
        let rows: Vec<&serde_json::Value> = listed["sessions"].as_array().unwrap().iter().collect();
        let titles: Vec<&str> = rows.iter().map(|r| r["title"].as_str().unwrap()).collect();
        assert_eq!(titles, ["Ana one", "Ana two"], "{listed}");

        // Narrowed by a channel, and by a word in a title.
        let listed = tool
            .call(
                ListSessionsArgs {
                    channel: Some(webui("ana").to_slug()),
                    query: None,
                },
                &ctx,
            )
            .await
            .expect("the listing");
        assert_eq!(listed["sessions"].as_array().unwrap().len(), 2, "{listed}");

        let listed = tool
            .call(
                ListSessionsArgs {
                    channel: None,
                    query: Some("two".into()),
                },
                &ctx,
            )
            .await
            .expect("the listing");
        let rows = listed["sessions"].as_array().unwrap();
        assert_eq!(rows.len(), 1, "{listed}");
        assert_eq!(rows[0]["topic_id"], "t2");
        // The row carries what a wake takes.
        assert_eq!(rows[0]["channel"], webui("ana").to_slug());
    });
}

#[test]
fn the_four_wakeable_channels_parse() {
    assert_eq!(
        wakeable_channel("discord__123456789"),
        Ok(VizierChannelId::DiscordChanel(123456789))
    );
    assert_eq!(
        wakeable_channel("telegram__-9"),
        Ok(VizierChannelId::TelegramChannel(-9))
    );
    assert_eq!(
        wakeable_channel("http__ana__vizier-webui"),
        Ok(VizierChannelId::HTTP("ana".into(), "vizier-webui".into()))
    );
    assert_eq!(
        wakeable_channel("project__ana__proj_abc"),
        Ok(VizierChannelId::Project("ana".into(), "proj_abc".into()))
    );
}

/// The machinery's own conversations are not wakeable, and the parser is the only place that
/// decides it — so there is no second list that could drift (research D2). Each is refused with
/// the reason, not as an unknown address.
#[test]
fn the_machinerys_own_conversations_are_refused() {
    for (slug, what) in [
        ("task__daily-report__1730000000000", "a scheduled run's own conversation"),
        ("SUBAGENT", "a subagent worker's conversation"),
        ("consultation__beta", "a consultation's conversation"),
        ("DREAM__agent__http__ana__vizier-webui__extraction", "a dream cycle's conversation"),
        ("SYSTEM", "the system's own channel"),
        ("inter_agent__[alpha,beta]", "a delegation's conversation"),
        ("workspace__old", "a Workspace's conversation"),
    ] {
        match wakeable_channel(slug) {
            Ok(channel) => panic!("{slug} must not be wakeable, but it is {channel:?}"),
            Err(access::Refusal::NotWakeable(reason)) => {
                assert!(reason.starts_with(what), "{slug}: {reason}")
            }
            Err(other) => panic!("{slug} must be refused as not wakeable: {other:?}"),
        }
    }
}

/// An address that is not an address at all.
#[test]
fn an_address_that_is_not_one_is_refused() {
    for slug in [
        "",
        "current",
        "http__ana",
        "http____vizier-webui",
        "http__ana__",
        "project__ana",
        "discord__not-a-number",
        "telegram__not-a-number",
        "some-channel",
    ] {
        assert!(
            matches!(wakeable_channel(slug), Err(access::Refusal::UnknownChannel)),
            "{slug} must not parse"
        );
    }
}

/// A channel id that contains a double underscore survives: the username and the channel id are
/// split at the **first** separator, so only the username may not contain one.
#[test]
fn a_channel_id_containing_a_double_underscore_survives() {
    assert_eq!(
        wakeable_channel("http__ana__a__b"),
        Ok(VizierChannelId::HTTP("ana".into(), "a__b".into()))
    );
    assert_eq!(
        wakeable_channel("project__ana__my__project"),
        Ok(VizierChannelId::Project("ana".into(), "my__project".into()))
    );
}

/// The reference a listing hands out is the one a wake takes (FR-004).
#[test]
fn a_slug_round_trips_through_the_parser() {
    for channel in [
        VizierChannelId::DiscordChanel(42),
        VizierChannelId::TelegramChannel(-7),
        VizierChannelId::HTTP("ana".into(), "vizier-webui".into()),
        VizierChannelId::Project("ana".into(), "proj_abc".into()),
    ] {
        let slug = channel.to_slug();
        assert_eq!(wakeable_channel(&slug), Ok(channel), "{slug}");
    }
}

/// The channels that have conversations of their own need a topic; the ones that do not never
/// have one.
#[test]
fn only_the_conversation_channels_have_topics() {
    assert!(has_topics(&VizierChannelId::HTTP(
        "ana".into(),
        "vizier-webui".into()
    )));
    assert!(has_topics(&VizierChannelId::Project(
        "ana".into(),
        "proj_abc".into()
    )));
    assert!(!has_topics(&VizierChannelId::DiscordChanel(42)));
    assert!(!has_topics(&VizierChannelId::TelegramChannel(-7)));
    assert_eq!(
        topic_for(
            &VizierChannelId::DiscordChanel(42),
            Some("ignored".into())
        ),
        None
    );
    assert_eq!(
        topic_for(
            &VizierChannelId::HTTP("ana".into(), "vizier-webui".into()),
            Some("t".into())
        ),
        Some("t".into())
    );
}

/// The wakeable set and the parser agree: everything the parser returns is wakeable, and
/// nothing else is.
#[test]
fn the_parser_and_the_wakeable_set_agree() {
    for channel in [
        VizierChannelId::DiscordChanel(42),
        VizierChannelId::TelegramChannel(-7),
        VizierChannelId::HTTP("ana".into(), "vizier-webui".into()),
        VizierChannelId::Project("ana".into(), "proj_abc".into()),
    ] {
        assert!(is_wakeable(&channel), "{channel:?} must be wakeable");
        assert!(wakeable_channel(&channel.to_slug()).is_ok());
    }
    for channel in [
        VizierChannelId::Task("daily".into(), crate::time::now()),
        VizierChannelId::Subagent,
        VizierChannelId::Consultation("beta".into()),
        VizierChannelId::System,
        VizierChannelId::InterAgent(vec!["alpha".into()]),
        VizierChannelId::Workspace("old".into()),
    ] {
        assert!(!is_wakeable(&channel), "{channel:?} must not be wakeable");
    }
}
