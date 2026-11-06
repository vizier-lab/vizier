//! The Workspace service against a real runtime: who may create, who may see, who may add whom
//! (`specs/053-workspace-channel/contracts/http-api.md`). The conformance vectors cannot cover
//! success paths, because the harness sends each vector twice.

use serde_json::json;
use vizier_core::{
    auth::{AuthMethod, Principal},
    config::VizierConfig,
    runtime::{RuntimeOptions, VizierRuntime},
    schema::AgentConfig,
    service::{
        ServiceError, Services,
        workspaces::{self as ws, AddMemberRequest, CreateTopicRequest, CreateWorkspaceRequest},
    },
    storage::{agent::AgentStorage, user::UserStorage},
};

fn agent(name: &str, owner: &str) -> AgentConfig {
    serde_json::from_value(json!({
        "name": name, "owner_id": owner, "system_prompt": null, "description": null,
        "provider": "dummyplug", "model": "dummyplug", "thinking_depth": 1,
        "tools": {"timeout": "30s"}, "silent_read_initiative_chance": 0.0,
        "prompt_timeout": "1m", "heartbeat_interval": "1m"
    }))
    .unwrap()
}

struct World {
    s: Services,
    _runtime: VizierRuntime,
    _dir: tempfile::TempDir,
    rafi: Principal,
    maya: Principal,
    eve: Principal,
    admin: Principal,
}

async fn person(s: &Services, name: &str, permissions: &[&str]) -> Principal {
    let storage = &s.deps().storage;
    let role = storage
        .create_role(name, permissions.iter().map(|p| p.to_string()).collect(), false)
        .await
        .unwrap();
    let user = storage.create_user(name, "", &role.role_id).await.unwrap();
    s.principal_for(&user.user_id, AuthMethod::Jwt).await.unwrap()
}

async fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let runtime = VizierRuntime::start(RuntimeOptions {
        config: VizierConfig::with_workspace(dir.path().to_path_buf()),
        ..Default::default()
    })
    .await
    .unwrap();
    let s = Services::new(runtime.deps().clone());
    let rafi = person(&s, "rafi", &["workspaces:create", "owned_agents:view", "owned_agents:edit"]).await;
    let maya = person(&s, "maya", &["owned_agents:view", "owned_agents:edit"]).await;
    let eve = person(&s, "eve", &["workspaces:create"]).await;
    let admin = person(&s, "ops", &["all_workspaces:manage"]).await;
    let storage = &s.deps().storage;
    storage.create_agent("alpha", &agent("Alpha", &rafi.user_id)).await.unwrap();
    // Someone else's agent, which rafi may neither see nor edit.
    storage.create_agent("beta", &agent("Beta", &maya.user_id)).await.unwrap();
    World { s, _runtime: runtime, _dir: dir, rafi, maya, eve, admin }
}

fn code(e: &ServiceError) -> Option<&str> {
    e.code()
}

async fn ops(w: &World) -> String {
    ws::create(&w.s, &w.rafi, CreateWorkspaceRequest { name: "ops".into(), first_topic: None })
        .await
        .unwrap()
        .workspace
        .id
}

fn add(kind: &str, handle: &str) -> AddMemberRequest {
    AddMemberRequest { kind: kind.into(), handle: handle.into(), role: None }
}

#[tokio::test(flavor = "multi_thread")]
async fn creating_needs_the_permission_and_starts_with_a_topic_and_an_owner() {
    let w = world().await;
    let denied = ws::create(&w.s, &w.maya, CreateWorkspaceRequest { name: "ops".into(), first_topic: None })
        .await
        .unwrap_err();
    assert_eq!(denied.status(), 403);

    let made = ws::create(&w.s, &w.rafi, CreateWorkspaceRequest { name: "ops".into(), first_topic: None })
        .await
        .unwrap();
    assert_eq!(made.workspace.role, "owner");
    assert_eq!(made.workspace.topics.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["general"]);
    assert_eq!(made.members.len(), 1);
    assert_eq!((made.members[0].handle.as_str(), made.members[0].role.as_str()), ("rafi", "owner"));
}

#[tokio::test(flavor = "multi_thread")]
async fn names_are_unique_ignoring_case_and_follow_the_rule() {
    let w = world().await;
    ops(&w).await;
    let clash = ws::create(&w.s, &w.eve, CreateWorkspaceRequest { name: "OPS".into(), first_topic: None })
        .await
        .unwrap_err();
    assert_eq!((clash.status(), code(&clash)), (409, Some("name_in_use")));

    let bad = ws::create(&w.s, &w.eve, CreateWorkspaceRequest { name: "Not Allowed!".into(), first_topic: None })
        .await
        .unwrap_err();
    assert_eq!((bad.status(), code(&bad)), (422, Some("invalid_name")));
    let bad_topic = ws::create(
        &w.s,
        &w.eve,
        CreateWorkspaceRequest { name: "fine".into(), first_topic: Some("No Spaces".into()) },
    )
    .await
    .unwrap_err();
    assert_eq!((bad_topic.status(), code(&bad_topic)), (422, Some("invalid_name")));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stranger_cannot_tell_a_workspace_exists_but_an_administrator_stands_in_it() {
    let w = world().await;
    let id = ops(&w).await;

    // Same answer for one that exists and one that does not, on every operation.
    for target in [id.as_str(), "no-such-workspace"] {
        let detail = ws::detail(&w.s, &w.eve, target).await.unwrap_err();
        let topic = ws::create_topic(&w.s, &w.eve, target, CreateTopicRequest { name: "x".into() })
            .await
            .unwrap_err();
        let member = ws::add_member(&w.s, &w.eve, target, add("agent", "alpha")).await.unwrap_err();
        let remove = ws::remove_member(&w.s, &w.eve, target, "person", "rafi").await.unwrap_err();
        for e in [&detail, &topic, &member, &remove] {
            assert_eq!(e.status(), 404, "{e}");
            assert_eq!(e.message(), "workspace not found");
        }
    }
    assert!(ws::list(&w.s, &w.eve).await.unwrap().is_empty());

    let seen = ws::detail(&w.s, &w.admin, &id).await.unwrap();
    assert_eq!(seen.workspace.role, "admin");
    // An administrator who is not a member is not in the list of members or of Workspaces.
    assert!(seen.members.iter().all(|m| m.handle != "ops"));
    assert!(ws::list(&w.s, &w.admin).await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn only_an_owner_adds_and_a_member_adds_nobody() {
    let w = world().await;
    let id = ops(&w).await;
    ws::add_member(&w.s, &w.rafi, &id, add("person", "maya")).await.unwrap();

    let by_member = ws::add_member(&w.s, &w.maya, &id, add("person", "eve")).await.unwrap_err();
    assert_eq!(by_member.status(), 403);
    // A member still sees it and may add a topic.
    assert_eq!(ws::detail(&w.s, &w.maya, &id).await.unwrap().workspace.role, "member");
    ws::create_topic(&w.s, &w.maya, &id, CreateTopicRequest { name: "daily".into() }).await.unwrap();
    let again = ws::create_topic(&w.s, &w.maya, &id, CreateTopicRequest { name: "Daily".into() })
        .await
        .unwrap_err();
    assert_eq!((again.status(), code(&again)), (409, Some("name_in_use")));
}

#[tokio::test(flavor = "multi_thread")]
async fn adding_an_agent_takes_the_right_to_edit_it() {
    let w = world().await;
    let id = ops(&w).await;

    let mine = ws::add_member(&w.s, &w.rafi, &id, add("agent", "alpha")).await.unwrap();
    assert_eq!((mine.name.as_str(), mine.role.as_str(), mine.available), ("Alpha", "member", true));

    // Maya's agent: rafi can neither see nor edit it.
    let refused = ws::add_member(&w.s, &w.rafi, &id, add("agent", "beta")).await.unwrap_err();
    assert_eq!((refused.status(), code(&refused)), (403, Some("cannot_edit_agent")));
    let missing = ws::add_member(&w.s, &w.rafi, &id, add("agent", "ghost")).await.unwrap_err();
    assert_eq!(missing.status(), 404);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_handle_names_one_member() {
    let w = world().await;
    let id = ops(&w).await;
    ws::add_member(&w.s, &w.rafi, &id, add("agent", "alpha")).await.unwrap();

    let twice = ws::add_member(&w.s, &w.rafi, &id, add("agent", "alpha")).await.unwrap_err();
    assert_eq!((twice.status(), code(&twice)), (409, Some("handle_in_use")));
    // A person called like the agent would make `@alpha` ambiguous.
    let storage = &w.s.deps().storage;
    let role = storage.create_role("plain", vec![], false).await.unwrap();
    storage.create_user("alpha", "", &role.role_id).await.unwrap();
    let clash = ws::add_member(&w.s, &w.rafi, &id, add("person", "alpha")).await.unwrap_err();
    assert_eq!((clash.status(), code(&clash)), (409, Some("handle_in_use")));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_last_owner_cannot_leave_but_an_administrator_can_remove_them() {
    let w = world().await;
    let id = ops(&w).await;
    let e = ws::remove_member(&w.s, &w.rafi, &id, "person", "rafi").await.unwrap_err();
    assert_eq!((e.status(), code(&e)), (409, Some("last_owner")));

    ws::add_member(&w.s, &w.rafi, &id, AddMemberRequest { kind: "person".into(), handle: "maya".into(), role: Some("owner".into()) })
        .await
        .unwrap();
    ws::remove_member(&w.s, &w.rafi, &id, "person", "rafi").await.unwrap();
    // Rafi is out, so the Workspace is gone from their point of view.
    assert_eq!(ws::detail(&w.s, &w.rafi, &id).await.unwrap_err().status(), 404);

    let last = ws::remove_member(&w.s, &w.maya, &id, "person", "maya").await.unwrap_err();
    assert_eq!(code(&last), Some("last_owner"));
    ws::remove_member(&w.s, &w.admin, &id, "person", "maya").await.unwrap();
}

// --- notifications (spec 053 US3) -------------------------------------------------------------------

use vizier_core::service::notifications as inbox;

async fn unread_of(w: &World, who: &Principal) -> u64 {
    inbox::count(&w.s, who).await.unwrap().unread
}

#[tokio::test(flavor = "multi_thread")]
async fn every_message_notifies_every_other_member_once_and_nobody_else() {
    let w = world().await;
    let id = ops(&w).await;
    ws::add_member(&w.s, &w.rafi, &id, add("person", "maya")).await.unwrap();
    let topic = ws::detail(&w.s, &w.rafi, &id).await.unwrap().workspace.topics[0].id.clone();

    let say = |text: &str| ws::PostRequest { text: text.into(), files: vec![] };
    ws::post(&w.s, &w.rafi, &id, &topic, say("hello team")).await.unwrap();

    // The member is told, with the topic as the place and a short excerpt.
    assert_eq!(unread_of(&w, &w.maya).await, 1);
    let page = inbox::list(&w.s, &w.maya, None, None, None).await.unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].category, vizier_core::schema::NotificationCategory::WorkspaceMessage);
    assert_eq!(page.items[0].excerpt.as_deref(), Some("hello team"));
    assert_eq!(page.items[0].href, format!("/workspaces/{id}/{topic}"));
    // Not its author, and not someone outside the Workspace.
    assert_eq!(unread_of(&w, &w.rafi).await, 0);
    assert_eq!(unread_of(&w, &w.eve).await, 0);
    assert_eq!(unread_of(&w, &w.admin).await, 0, "an administrator who is not a member is not notified");

    // A command leaves a note, and a note notifies nobody.
    ws::post(&w.s, &w.rafi, &id, &topic, say("/abort")).await.unwrap();
    assert_eq!(unread_of(&w, &w.maya).await, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn unread_is_counted_per_topic_and_clears_when_the_topic_is_read() {
    let w = world().await;
    let id = ops(&w).await;
    ws::add_member(&w.s, &w.rafi, &id, add("person", "maya")).await.unwrap();
    let general = ws::detail(&w.s, &w.rafi, &id).await.unwrap().workspace.topics[0].id.clone();
    let daily = ws::create_topic(&w.s, &w.rafi, &id, CreateTopicRequest { name: "daily".into() }).await.unwrap().id;
    let say = |text: &str| ws::PostRequest { text: text.into(), files: vec![] };
    for _ in 0..2 {
        ws::post(&w.s, &w.rafi, &id, &general, say("one")).await.unwrap();
    }
    ws::post(&w.s, &w.rafi, &id, &daily, say("two")).await.unwrap();

    let before = ws::list(&w.s, &w.maya).await.unwrap().remove(0);
    assert_eq!(before.unread, 3);
    let by_topic: std::collections::BTreeMap<_, _> = before.topics.iter().map(|t| (t.name.clone(), t.unread)).collect();
    assert_eq!(by_topic, [("daily".to_string(), 1), ("general".to_string(), 2)].into());
    // The author sees none of her own.
    assert_eq!(ws::list(&w.s, &w.rafi).await.unwrap()[0].unread, 0);

    assert_eq!(ws::mark_topic_read(&w.s, &w.maya, &id, &general).await.unwrap(), 2);
    let after = ws::list(&w.s, &w.maya).await.unwrap().remove(0);
    assert_eq!((after.unread, after.topics.iter().find(|t| t.name == "daily").unwrap().unread), (1, 1));
    assert_eq!(unread_of(&w, &w.maya).await, 1);
    // Reading again changes nothing, and a stranger cannot read it.
    assert_eq!(ws::mark_topic_read(&w.s, &w.maya, &id, &general).await.unwrap(), 0);
    assert_eq!(ws::mark_topic_read(&w.s, &w.eve, &id, &general).await.unwrap_err().status(), 404);
}

#[tokio::test(flavor = "multi_thread")]
async fn removing_a_member_takes_their_notifications_away_at_once() {
    let w = world().await;
    let id = ops(&w).await;
    ws::add_member(&w.s, &w.rafi, &id, add("person", "maya")).await.unwrap();
    let topic = ws::detail(&w.s, &w.rafi, &id).await.unwrap().workspace.topics[0].id.clone();
    ws::post(&w.s, &w.rafi, &id, &topic, ws::PostRequest { text: "secret plans".into(), files: vec![] }).await.unwrap();
    assert_eq!(unread_of(&w, &w.maya).await, 1);

    ws::remove_member(&w.s, &w.rafi, &id, "person", "maya").await.unwrap();
    assert_eq!(unread_of(&w, &w.maya).await, 0);
    assert!(inbox::list(&w.s, &w.maya, None, None, None).await.unwrap().items.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_is_notified_about_an_agent_they_cannot_view() {
    let w = world().await;
    let id = ops(&w).await;
    // Eve can neither view nor edit alpha, and is added as a person.
    ws::add_member(&w.s, &w.rafi, &id, add("person", "eve")).await.unwrap();
    ws::add_member(&w.s, &w.rafi, &id, add("agent", "alpha")).await.unwrap();
    let topic = ws::detail(&w.s, &w.rafi, &id).await.unwrap().workspace.topics[0].id.clone();
    // Alpha's text is written by its turn; here a person's message stands in for the agent's.
    ws::post(&w.s, &w.rafi, &id, &topic, ws::PostRequest { text: "@alpha status?".into(), files: vec![] }).await.unwrap();
    assert_eq!(unread_of(&w, &w.eve).await, 1);
}
