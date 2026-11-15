//! Recovery on a claim (`specs/057-fleet-row-ownership` US1, US3–US5). One process stands in for
//! two owners: claiming a scope again gives a newer fence, so the rows opened under the first
//! claim are the ones a new owner recovers. The two-store stale-writer cases are in
//! `storage/sqlite/fence_tests.rs`.

use std::sync::Arc;

use crate::config::VizierConfig;
use crate::dependencies::VizierDependencies;
use crate::recovery::{recover_agent, recover_control};
use crate::schema::{
    Canceller, ClientCallOutcome, DetachedScript, DetachedScriptState, TaskRunState,
    VizierChannelId, VizierRequest, VizierRequestContent, VizierSession,
};
use crate::storage::detached_script::DetachedScriptStorage;
use crate::storage::client_call::ClientCallStorage;
use crate::storage::fence::{Fence, FenceStorage, Scope};
use crate::storage::question::SensitiveAskStorage;
use crate::storage::scratch::ScratchStorage;
use crate::storage::task_run::{TaskRunStorage, task_run_session};
use crate::storage::turn::{TurnState, TurnStorage};

async fn setup() -> (VizierDependencies, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let config = VizierConfig {
        workspace: dir.path().to_string_lossy().to_string(),
        ..Default::default()
    };
    (VizierDependencies::new(config).await.unwrap(), dir)
}

async fn claim(deps: &VizierDependencies, scope: &Scope) -> Fence {
    deps.storage.claim(scope, "local", None).await.unwrap()
}

fn session(agent: &str, topic: &str) -> VizierSession {
    VizierSession(
        agent.into(),
        VizierChannelId::HTTP("u".into(), "webui".into()),
        Some(topic.into()),
    )
}

fn script(id: &str, session: VizierSession) -> DetachedScript {
    DetachedScript {
        id: id.into(),
        agent_id: session.0.clone(),
        session,
        parent_id: None,
        depth: 0,
        intent: "i".into(),
        state: DetachedScriptState::Running,
        timeout_secs: 600,
        code: "1".into(),
        tool_calls: serde_json::json!([]),
        created_at: crate::time::now(),
        finished_at: None,
        outcome: None,
        delivered_at: None,
        cancelled_by: None,
    }
}

async fn script_state(deps: &VizierDependencies, id: &str) -> DetachedScriptState {
    deps.storage.get_detached_script(id).await.unwrap().unwrap().state
}

/// The delivery pokes waiting in the agent's queue, by session (recovery spawns them).
async fn pokes(inbox: &crate::transport::Inbox, expected: usize) -> Vec<VizierSession> {
    let mut seen = vec![];
    while seen.len() < expected {
        let envelope = tokio::time::timeout(std::time::Duration::from_secs(5), inbox.recv_request())
            .await
            .expect("a delivery poke")
            .unwrap();
        if matches!(envelope.request.content, VizierRequestContent::DetachedDelivery(_)) {
            seen.push(envelope.session);
        }
    }
    seen
}

#[tokio::test]
async fn takeover_resends_exactly_expired_turns() {
    let (deps, _dir) = setup().await;
    let inbox = deps.transport.register_agent("x".into()).await;
    let first = claim(&deps, &Scope::Agent("x".into())).await;
    let c = claim(&deps, &Scope::Agent("y".into())).await;
    let queued = deps
        .storage
        .accept_turn(&session("x", "a"), &VizierRequest::default())
        .await
        .unwrap();
    let running = deps
        .storage
        .accept_turn(&session("x", "b"), &VizierRequest::default())
        .await
        .unwrap();
    deps.storage.start_turn(running).await.unwrap();
    // y's owner is alive: its turn is already taken and must stay where it is.
    let others = deps
        .storage
        .accept_turn(&session("y", "a"), &VizierRequest::default())
        .await
        .unwrap();
    assert!(deps.storage.claim_turn_on_receipt(others).await.unwrap());
    assert!(deps.storage.claim_turn_on_receipt(running).await.unwrap());
    assert!(first.token < claim(&deps, &Scope::Agent("x".into())).await.token);

    let fence = claim(&deps, &Scope::Agent("x".into())).await;
    let done = recover_agent(&deps, &"x".to_string(), &fence).await;
    assert!(done.complete);

    let mut seen = vec![];
    for _ in 0..2 {
        let envelope = tokio::time::timeout(std::time::Duration::from_secs(2), inbox.recv_request())
            .await
            .expect("a re-sent turn")
            .unwrap();
        seen.push((envelope.turn_id.unwrap(), envelope.resumed));
    }
    seen.sort();
    assert_eq!(seen, vec![(queued, false), (running, true)]);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(200), inbox.recv_request())
            .await
            .is_err(),
        "nothing else is sent"
    );
    assert_eq!(deps.storage.expired_turns("y", c.token).await.unwrap().len(), 0);
}

#[tokio::test]
async fn the_resume_counter_survives_a_change_of_owner() {
    let (deps, _dir) = setup().await;
    let scope = Scope::Agent("x".into());
    claim(&deps, &scope).await;
    let id = deps
        .storage
        .accept_turn(&session("x", "a"), &VizierRequest::default())
        .await
        .unwrap();
    let mut counts = vec![];
    for _ in 0..4 {
        // A new owner takes the turn on receipt and resumes it without progress.
        claim(&deps, &scope).await;
        assert!(deps.storage.claim_turn_on_receipt(id).await.unwrap());
        counts.push(deps.storage.note_resume(id, 2).await.unwrap());
    }
    assert_eq!(counts, vec![1, 2, 3, 4]);
    assert!(crate::agents::steps::exceeds_resume_limit(*counts.last().unwrap()));
}

#[tokio::test]
async fn a_turn_is_claimed_once_on_receipt() {
    let (deps, _dir) = setup().await;
    claim(&deps, &Scope::Agent("x".into())).await;
    let id = deps
        .storage
        .accept_turn(&session("x", "a"), &VizierRequest::default())
        .await
        .unwrap();
    assert!(deps.storage.claim_turn_on_receipt(id).await.unwrap());
    assert!(!deps.storage.claim_turn_on_receipt(id).await.unwrap(), "a duplicate");
    // A new owner may take it again: its token is newer.
    claim(&deps, &Scope::Agent("x".into())).await;
    assert!(deps.storage.claim_turn_on_receipt(id).await.unwrap());
    // A turn that finished meanwhile has nothing left to run.
    deps.storage.finish_turn(id).await.unwrap();
    assert!(!deps.storage.claim_turn_on_receipt(id).await.unwrap());
}

#[tokio::test]
async fn racing_recoverers_claim_each_turn_once() {
    let (deps, _dir) = setup().await;
    claim(&deps, &Scope::Agent("x".into())).await;
    let mut ids = vec![];
    for i in 0..1000 {
        ids.push(
            deps.storage
                .accept_turn(&session("x", &format!("t{i}")), &VizierRequest::default())
                .await
                .unwrap(),
        );
    }
    // Two recoverers re-send every row, so each is delivered twice; the owner claims on receipt.
    let ids = Arc::new(ids);
    let mut tasks = vec![];
    for _ in 0..2 {
        let (deps, ids) = (deps.clone(), ids.clone());
        tasks.push(tokio::spawn(async move {
            let mut won = 0;
            for id in ids.iter() {
                if deps.storage.claim_turn_on_receipt(*id).await.unwrap() {
                    won += 1;
                }
            }
            won
        }));
    }
    let mut total = 0;
    for task in tasks {
        total += task.await.unwrap();
    }
    assert_eq!(total, 1000, "every row is received and claimed exactly once");
}

#[tokio::test]
async fn scratch_kept_for_pending_turn_cleared_otherwise() {
    let (deps, _dir) = setup().await;
    let scope = Scope::Agent("x".into());
    claim(&deps, &scope).await;
    let (busy, idle, other) = (session("x", "busy"), session("x", "idle"), session("y", "t"));
    for s in [&busy, &idle, &other] {
        deps.storage.write_scratch(s, "n", "text").await.unwrap();
    }
    deps.storage
        .accept_turn(&busy, &VizierRequest::default())
        .await
        .unwrap();
    let fence = claim(&deps, &scope).await;
    let _inbox = deps.transport.register_agent("x".into()).await;
    assert!(recover_agent(&deps, &"x".to_string(), &fence).await.complete);
    assert_eq!(deps.storage.scratch_names(&busy).await.unwrap(), vec!["n"]);
    assert!(deps.storage.scratch_names(&idle).await.unwrap().is_empty());
    assert_eq!(
        deps.storage.scratch_names(&other).await.unwrap(),
        vec!["n"],
        "another agent's files are not touched"
    );
}

#[tokio::test]
async fn control_takeover_interrupts_unbacked_runs_once_and_keeps_backed_ones() {
    let (deps, _dir) = setup().await;
    claim(&deps, &Scope::Control).await;
    let now = crate::time::now();
    let lost = deps
        .storage
        .open_task_run("x".into(), "lost".into(), now, "k1".into())
        .await
        .unwrap();
    let backed = deps
        .storage
        .open_task_run("x".into(), "backed".into(), now, "k2".into())
        .await
        .unwrap();
    let run = deps.storage.get_task_run("x".into(), "backed".into(), now).await.unwrap().unwrap();
    deps.storage
        .accept_turn(&task_run_session(&run), &VizierRequest::default())
        .await
        .unwrap();

    let fence = claim(&deps, &Scope::Control).await;
    assert!(recover_control(&deps, &fence).await.complete);
    let state = |id: i64| {
        let deps = deps.clone();
        async move {
            deps.storage
                .open_task_runs()
                .await
                .unwrap()
                .iter()
                .any(|r| r.id == id)
        }
    };
    assert!(!state(lost).await, "the unbacked run was interrupted");
    assert!(state(backed).await, "the backed run stays open for the scheduler");
    let run = deps.storage.get_task_run("x".into(), "lost".into(), now).await.unwrap().unwrap();
    assert_eq!(run.state, TaskRunState::Interrupted);

    // A second pass resolves nothing twice: both rows now carry the new fence.
    assert!(deps.storage.expired_task_runs(fence.token).await.unwrap().is_empty());
    assert!(recover_control(&deps, &fence).await.complete);
}

/// Spec 058: a new owner interrupts the scripts its predecessor left running, once, never
/// touches a cancelled one or another agent's, and pokes each conversation with work to deliver.
#[tokio::test]
async fn agent_takeover_interrupts_running_scripts_once_and_pokes_their_conversations() {
    let (deps, _dir) = setup().await;
    let scope = Scope::Agent("x".into());
    claim(&deps, &scope).await;
    claim(&deps, &Scope::Agent("z".into())).await;
    deps.storage.open_detached_script(script("d_lost", session("x", "a"))).await.unwrap();
    deps.storage.open_detached_script(script("d_gone", session("x", "a"))).await.unwrap();
    deps.storage
        .cancel_detached_script("d_gone", Canceller::Agent("x".into()), crate::time::now())
        .await
        .unwrap();
    deps.storage.open_detached_script(script("d_done", session("x", "b"))).await.unwrap();
    deps.storage
        .close_detached_script(
            "d_done",
            DetachedScriptState::Succeeded,
            serde_json::json!({"ok": true}),
            serde_json::json!([]),
            crate::time::now(),
        )
        .await
        .unwrap();
    deps.storage.open_detached_script(script("d_elsewhere", session("z", "a"))).await.unwrap();

    let fence = claim(&deps, &scope).await;
    let inbox = deps.transport.register_agent("x".into()).await;
    assert!(recover_agent(&deps, &"x".to_string(), &fence).await.complete);

    assert_eq!(script_state(&deps, "d_lost").await, DetachedScriptState::Interrupted);
    assert_eq!(script_state(&deps, "d_gone").await, DetachedScriptState::Cancelled);
    assert_eq!(script_state(&deps, "d_elsewhere").await, DetachedScriptState::Running);
    let mut poked = pokes(&inbox, 2).await;
    poked.sort_by_key(|s| s.2.clone());
    assert_eq!(poked, vec![session("x", "a"), session("x", "b")]);

    // Nothing is left to interrupt a second time.
    assert!(recover_agent(&deps, &"x".to_string(), &fence).await.complete);
    assert!(
        deps.storage
            .interrupt_running_scripts("x", Some(fence.token), None)
            .await
            .unwrap()
            .is_empty()
    );
}

/// SC-002: fifty scripts running across several conversations when the owner died are each
/// interrupted once and delivered exactly once.
#[tokio::test]
async fn fifty_interrupted_scripts_are_each_delivered_once() {
    let (deps, _dir) = setup().await;
    let scope = Scope::Agent("x".into());
    claim(&deps, &scope).await;
    for i in 0..50 {
        let topic = format!("t{}", i % 5);
        deps.storage.open_detached_script(script(&format!("d_{i:02}"), session("x", &topic))).await.unwrap();
    }
    let fence = claim(&deps, &scope).await;
    let inbox = deps.transport.register_agent("x".into()).await;
    assert!(recover_agent(&deps, &"x".to_string(), &fence).await.complete);

    let sessions = pokes(&inbox, 5).await;
    let mut delivered = vec![];
    // Every poke handled, and every session claimed twice: the second claim finds nothing.
    for session in sessions.iter().chain(sessions.iter()) {
        delivered.extend(deps.detached.claim(session).await.into_iter().map(|s| s.id));
    }
    delivered.sort();
    assert_eq!(delivered.len(), 50);
    delivered.dedup();
    assert_eq!(delivered.len(), 50, "a script was delivered twice");
}

#[tokio::test]
async fn agent_takeover_interrupts_pending_client_calls_and_abandons_asks() {
    use crate::questions::model::{AskState, SensitiveAskRecord};
    let (deps, _dir) = setup().await;
    let scope = Scope::Agent("x".into());
    claim(&deps, &scope).await;
    claim(&deps, &Scope::Agent("y".into())).await;
    let ask = |id: &str, agent: &str| SensitiveAskRecord {
        ask_id: id.into(),
        purpose: "a token".into(),
        label: None,
        session: session(agent, "t"),
        state: AskState::Open,
        requested_at: crate::time::now(),
        closed_at: None,
    };
    deps.storage.open_client_call("pending", &session("x", "t")).await.unwrap();
    deps.storage.open_client_call("deferred", &session("x", "t")).await.unwrap();
    deps.storage.defer_client_call("deferred").await.unwrap();
    deps.storage.open_client_call("other", &session("y", "t")).await.unwrap();
    deps.storage.open_sensitive_ask(&ask("a1", "x")).await.unwrap();
    deps.storage.open_sensitive_ask(&ask("a2", "y")).await.unwrap();

    let fence = claim(&deps, &scope).await;
    assert!(recover_agent(&deps, &"x".to_string(), &fence).await.complete);

    // The pending call was interrupted (a late outcome changes nothing), the deferred call is
    // still open and finishes once, and y's call is untouched.
    assert!(
        !deps
            .storage
            .finish_client_call("pending", &ClientCallOutcome::Cancelled)
            .await
            .unwrap()
    );
    assert!(
        deps.storage
            .finish_client_call("deferred", &ClientCallOutcome::Cancelled)
            .await
            .unwrap()
    );
    assert!(
        deps.storage
            .finish_client_call("other", &ClientCallOutcome::Cancelled)
            .await
            .unwrap()
    );
    let asks = deps.storage.open_sensitive_asks_all().await.unwrap();
    assert_eq!(asks.len(), 1);
    assert_eq!(asks[0].ask_id, "a2", "y's ask is still open");
}

/// One start of a single node produces, for each kind of in-flight record, the outcome the old
/// startup sweeps did.
#[tokio::test]
async fn single_node_restart_outcome_unchanged() {
    let (deps, _dir) = setup().await;
    let scope = Scope::Agent("x".into());
    // The first process.
    claim(&deps, &scope).await;
    claim(&deps, &Scope::Control).await;
    let now = crate::time::now();
    let id = deps
        .storage
        .accept_turn(&session("x", "t"), &VizierRequest::default())
        .await
        .unwrap();
    deps.storage.claim_turn_on_receipt(id).await.unwrap();
    deps.storage.start_turn(id).await.unwrap();
    deps.storage.write_scratch(&session("x", "t"), "n", "k").await.unwrap();
    deps.storage.write_scratch(&session("x", "gone"), "n", "k").await.unwrap();
    deps.storage.open_client_call("c", &session("x", "t")).await.unwrap();
    deps.storage.open_detached_script(script("d_j", session("x", "t"))).await.unwrap();
    let run_id = deps
        .storage
        .open_task_run("x".into(), "r".into(), now, "k".into())
        .await
        .unwrap();

    // The second process claims every scope once.
    let control = claim(&deps, &Scope::Control).await;
    let fence = claim(&deps, &scope).await;
    let inbox = deps.transport.register_agent("x".into()).await;
    assert!(recover_control(&deps, &control).await.complete);
    assert!(recover_agent(&deps, &"x".to_string(), &fence).await.complete);

    let envelope = inbox.recv_request().await.unwrap();
    assert_eq!((envelope.turn_id, envelope.resumed), (Some(id), true), "resumed");
    assert_eq!(deps.storage.scratch_names(&session("x", "t")).await.unwrap(), vec!["n"]);
    assert!(deps.storage.scratch_names(&session("x", "gone")).await.unwrap().is_empty());
    assert!(
        !deps
            .storage
            .finish_client_call("c", &ClientCallOutcome::Cancelled)
            .await
            .unwrap(),
        "the pending call was interrupted"
    );
    assert_eq!(script_state(&deps, "d_j").await, DetachedScriptState::Interrupted);
    let run = deps.storage.get_task_run("x".into(), "r".into(), now).await.unwrap().unwrap();
    assert_eq!(run.state, TaskRunState::Interrupted);
    assert_eq!(run.id, run_id);
    let _ = TurnState::Queued;
}

/// A row written before ownership existed carries no owner, and the first claim recovers it.
#[tokio::test]
async fn legacy_rows_recovered_once() {
    let (deps, _dir) = setup().await;
    // No claim yet: the writes are unfenced and the rows carry a null owner.
    deps.storage.open_client_call("c", &session("x", "t")).await.unwrap();
    let fence = claim(&deps, &Scope::Agent("x".into())).await;
    assert_eq!(deps.storage.interrupt_expired_client_calls("x", fence.token).await.unwrap(), 1);
    assert_eq!(
        deps.storage.interrupt_expired_client_calls("x", fence.token).await.unwrap(),
        0,
        "a second pass finds it re-owned"
    );
}

/// A single node never has a write refused: it claims once and nothing can claim over it.
#[tokio::test]
async fn single_node_never_refused() {
    use crate::storage::history::HistoryStorage;
    let (deps, _dir) = setup().await;
    claim(&deps, &Scope::Agent("x".into())).await;
    claim(&deps, &Scope::Control).await;
    let s = session("x", "t");
    let id = deps.storage.accept_turn(&s, &VizierRequest::default()).await.unwrap();
    deps.storage.claim_turn_on_receipt(id).await.unwrap();
    deps.storage.start_turn(id).await.unwrap();
    deps.storage
        .save_session_history(
            s.clone(),
            crate::schema::SessionHistoryContent::Request(VizierRequest::default()),
        )
        .await
        .unwrap();
    deps.storage.open_client_call("c", &s).await.unwrap();
    deps.storage.finish_client_call("c", &ClientCallOutcome::Cancelled).await.unwrap();
    deps.storage.open_detached_script(script("d_j", session("x", "t"))).await.unwrap();
    deps.storage
        .open_task_run("x".into(), "r".into(), crate::time::now(), "k".into())
        .await
        .unwrap();
    deps.storage.finish_turn(id).await.unwrap();
    assert!(!*deps.storage.ownership_lost(&Scope::Agent("x".into())).borrow());
}
