//! Stale-owner tests (`specs/057-fleet-row-ownership` US2, SC-002): once B claims a scope, every
//! fenced write of A is refused whole, and the same write by B succeeds. Each test runs on SQLite
//! and, through `postgres_tests!`, on two pools sharing one PostgreSQL schema.

use std::sync::Arc;

use super::SqlStorage;
use crate::schema::{
    ClientCallOutcome, SessionHistoryContent, TaskRunState, VizierChannelId, VizierRequest,
    VizierRequestContent, VizierSession,
};
use crate::storage::client_call::ClientCallStorage;
use crate::storage::detached_script::{CancelOutcome, DetachedScriptStorage};
use crate::storage::fence::{FenceStorage, Scope, is_ownership_lost};
use crate::storage::history::HistoryStorage;
use crate::storage::question::SensitiveAskStorage;
use crate::storage::sql::{SqlStore, params};
use crate::storage::subagent::{Settled, StopOutcome, SubagentStorage};
use crate::storage::task_run::TaskRunStorage;
use crate::storage::turn::{TurnState, TurnStorage};
use crate::tests_support::{node_storage, test_store_pair};

fn sqlite() -> SqlStore {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    SqlStorage::init_schema(&conn).unwrap();
    SqlStore::sqlite(conn)
}

struct Nodes {
    a: SqlStorage,
    b: SqlStorage,
    _dirs: (tempfile::TempDir, tempfile::TempDir),
}

/// Two nodes of one database. `claim_a` has A claim `scope` first.
fn nodes() -> Nodes {
    let (sa, sb) = test_store_pair(sqlite);
    let ((a, da), (b, db)) = (node_storage(sa), node_storage(sb));
    Nodes {
        a,
        b,
        _dirs: (da, db),
    }
}

impl Nodes {
    async fn a_then_b(&self, scope: &Scope) {
        self.a.claim(scope, "a", None).await.unwrap();
    }

    /// B takes over: from here A is a former owner.
    async fn b_takes_over(&self, scope: &Scope) {
        self.b.claim(scope, "b", None).await.unwrap();
    }

    fn assert_refused<T: std::fmt::Debug>(&self, scope: &Scope, result: anyhow::Result<T>) {
        let err = result.expect_err("a former owner's write must be refused");
        assert!(is_ownership_lost(&err), "{err:#}");
        assert!(*self.a.ownership_lost(scope).borrow());
    }
}

fn session(agent: &str) -> VizierSession {
    VizierSession(
        agent.into(),
        VizierChannelId::HTTP("u".into(), "webui".into()),
        Some("t".into()),
    )
}

fn request(text: &str) -> SessionHistoryContent {
    SessionHistoryContent::Request(VizierRequest {
        content: VizierRequestContent::Chat(text.into()),
        ..Default::default()
    })
}

fn scalar(storage: &SqlStorage, sql: &str, key: &str) -> Option<String> {
    storage
        .conn
        .lock()
        .unwrap()
        .query_all(sql, params![key])
        .unwrap()
        .first()
        .map(|row| row.get::<_, Option<String>>(0).unwrap().unwrap_or_default())
}

#[tokio::test]
async fn stale_history_save_refused() {
    let n = nodes();
    let scope = Scope::Agent("x".into());
    n.a_then_b(&scope).await;
    n.a.save_session_history(session("x"), request("before"))
        .await
        .unwrap();
    n.b_takes_over(&scope).await;

    n.assert_refused(
        &scope,
        n.a.save_session_history(session("x"), request("late")).await,
    );
    n.assert_refused(&scope, n.a.save_checkpoint(session("x"), None).await);
    let rows = n
        .b
        .list_session_history(session("x"), None, None, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "the refused writes left nothing");

    n.b.save_session_history(session("x"), request("b's"))
        .await
        .unwrap();
    n.b.save_checkpoint(session("x"), None).await.unwrap();
    assert_eq!(
        n.b.list_session_history(session("x"), None, None, None)
            .await
            .unwrap()
            .len(),
        3
    );
}

#[tokio::test]
async fn stale_turn_transition_refused() {
    let n = nodes();
    let scope = Scope::Agent("x".into());
    n.a_then_b(&scope).await;
    let id = n
        .a
        .accept_turn(&session("x"), &VizierRequest::default())
        .await
        .unwrap();
    n.b_takes_over(&scope).await;

    n.assert_refused(&scope, n.a.start_turn(id).await);
    n.assert_refused(&scope, n.a.set_turn_request_uid(id, "uid").await);
    n.assert_refused(&scope, n.a.note_resume(id, 1).await);
    n.assert_refused(&scope, n.a.finish_turn(id).await);
    n.assert_refused(&scope, n.a.cancel_session_turns(&session("x")).await);
    let rows = n.b.pending_turns("x").await.unwrap();
    assert_eq!(rows.len(), 1, "the row survived");
    assert_eq!(rows[0].state, TurnState::Queued);
    assert_eq!(rows[0].request_uid, None);

    n.b.start_turn(id).await.unwrap();
    n.b.set_turn_request_uid(id, "uid").await.unwrap();
    assert_eq!(n.b.note_resume(id, 1).await.unwrap(), 1);
    n.b.finish_turn(id).await.unwrap();
    assert!(n.b.pending_turns("x").await.unwrap().is_empty());
}

#[tokio::test]
async fn stale_client_call_write_refused() {
    let n = nodes();
    let scope = Scope::Agent("x".into());
    n.a_then_b(&scope).await;
    n.a.open_client_call("c1", &session("x")).await.unwrap();
    n.b_takes_over(&scope).await;
    let status = |uid: &str| {
        scalar(
            &n.b,
            "SELECT status FROM client_tool_call WHERE history_uid = ?1",
            uid,
        )
    };

    n.assert_refused(&scope, n.a.open_client_call("c2", &session("x")).await);
    assert_eq!(status("c2"), None);
    n.assert_refused(
        &scope,
        n.a.finish_client_call("c1", &ClientCallOutcome::Cancelled)
            .await,
    );
    n.assert_refused(&scope, n.a.defer_client_call("c1").await);
    n.assert_refused(
        &scope,
        n.a.save_client_call_state("c1", &serde_json::json!({"k": 1}))
            .await,
    );
    assert_eq!(status("c1").as_deref(), Some("pending"));
    assert_eq!(
        scalar(
            &n.b,
            "SELECT state FROM client_tool_call WHERE history_uid = ?1",
            "c1"
        ),
        Some(String::new()),
        "no state was saved (a null state reads as empty here)"
    );

    n.b.save_client_call_state("c1", &serde_json::json!({"k": 1}))
        .await
        .unwrap();
    assert!(n.b.defer_client_call("c1").await.unwrap());
    assert!(
        n.b.finish_client_call("c1", &ClientCallOutcome::Cancelled)
            .await
            .unwrap()
    );
    n.b.open_client_call("c2", &session("x")).await.unwrap();
    assert_eq!(status("c2").as_deref(), Some("pending"));
}

#[tokio::test]
async fn stale_sensitive_ask_write_refused() {
    use crate::questions::model::{AskState, SensitiveAskRecord};
    let n = nodes();
    let scope = Scope::Agent("x".into());
    let ask = |id: &str| SensitiveAskRecord {
        ask_id: id.into(),
        purpose: "a token".into(),
        label: None,
        session: session("x"),
        state: AskState::Open,
        requested_at: crate::time::now(),
        closed_at: None,
    };
    n.a_then_b(&scope).await;
    n.a.open_sensitive_ask(&ask("s1")).await.unwrap();
    n.b_takes_over(&scope).await;

    n.assert_refused(&scope, n.a.open_sensitive_ask(&ask("s2")).await);
    n.assert_refused(&scope, n.a.close_sensitive_ask("s1", AskState::Abandoned).await);
    let open = n.b.open_sensitive_asks_all().await.unwrap();
    assert_eq!(open.len(), 1, "s1 stays open, s2 was never written");

    assert!(
        n.b.close_sensitive_ask("s1", AskState::Abandoned)
            .await
            .unwrap()
    );
    n.b.open_sensitive_ask(&ask("s2")).await.unwrap();
}

fn detached(id: &str, parent: Option<&str>) -> crate::schema::DetachedScript {
    crate::schema::DetachedScript {
        id: id.into(),
        agent_id: "x".into(),
        session: session("x"),
        parent_id: parent.map(str::to_string),
        depth: 0,
        intent: "i".into(),
        state: crate::schema::DetachedScriptState::Running,
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

/// Spec 058: every write that advances a detached script is fenced on its agent's scope.
#[tokio::test]
async fn stale_detached_script_write_refused() {
    use crate::schema::{Canceller, DetachedScriptState};
    let n = nodes();
    let scope = Scope::Agent("x".into());
    n.a_then_b(&scope).await;
    n.a.open_detached_script(detached("d_1", None)).await.unwrap();
    n.a.open_detached_script(detached("d_2", None)).await.unwrap();
    n.b_takes_over(&scope).await;
    let now = crate::time::now();
    let done = || serde_json::json!({"ok": true});

    n.assert_refused(&scope, n.a.open_detached_script(detached("d_3", None)).await);
    n.assert_refused(&scope, n.a.record_detached_progress("d_1", serde_json::json!([1])).await);
    n.assert_refused(
        &scope,
        n.a.close_detached_script("d_1", DetachedScriptState::Succeeded, done(), serde_json::json!([]), now)
            .await,
    );
    n.assert_refused(
        &scope,
        n.a.cancel_detached_script("d_2", Canceller::Agent("x".into()), now).await,
    );
    n.assert_refused(&scope, n.a.claim_deliverable_scripts(&session("x")).await);
    n.assert_refused(&scope, n.a.interrupt_running_scripts("x", None, None).await);
    assert!(n.b.get_detached_script("d_3").await.unwrap().is_none());
    let kept = n.b.get_detached_script("d_1").await.unwrap().unwrap();
    assert_eq!(kept.state, DetachedScriptState::Running);
    assert_eq!(kept.tool_call_count(), 0);

    // The same writes by the new owner succeed.
    n.b.open_detached_script(detached("d_3", None)).await.unwrap();
    n.b.record_detached_progress("d_1", serde_json::json!([1])).await.unwrap();
    assert!(
        n.b.close_detached_script("d_1", DetachedScriptState::Succeeded, done(), serde_json::json!([]), now)
            .await
            .unwrap()
    );
    assert_eq!(
        n.b.cancel_detached_script("d_2", Canceller::Agent("x".into()), now).await.unwrap(),
        CancelOutcome::Cancelled(vec!["d_2".into()])
    );
    assert_eq!(n.b.claim_deliverable_scripts(&session("x")).await.unwrap().len(), 1);
    assert_eq!(n.b.interrupt_running_scripts("x", None, None).await.unwrap().len(), 1);
}

fn worker(id: &str) -> crate::schema::Subagent {
    let mut row = crate::storage::sqlite::subagent::tests::worker(id, session("x"), 1);
    row.agent_id = "x".into();
    row
}

/// Spec 059: every write that advances a worker is fenced on its agent's scope, one test row per
/// fenced `SubagentStorage` method.
#[tokio::test]
async fn stale_subagent_write_refused() {
    use crate::schema::{SubagentState, SubagentStopReason};
    let n = nodes();
    let scope = Scope::Agent("x".into());
    n.a_then_b(&scope).await;
    n.a.open_subagents(vec![worker("s_1"), worker("s_2"), worker("s_3")]).await.unwrap();
    n.a.ask_subagent("s_2", "which?", 1_000, crate::time::now()).await.unwrap().unwrap();
    n.b_takes_over(&scope).await;
    let now = crate::time::now();
    let answered = || Settled::Answered { result: "done".into(), truncated: false };

    n.assert_refused(&scope, n.a.open_subagents(vec![worker("s_4")]).await);
    n.assert_refused(&scope, n.a.ask_subagent("s_1", "q", 0, now).await);
    n.assert_refused(&scope, n.a.reply_subagent("s_2", now, now).await);
    n.assert_refused(&scope, n.a.settle_subagent("s_1", answered(), now).await);
    n.assert_refused(&scope, n.a.stop_subagent("s_3", SubagentStopReason::ByAgent, now).await);
    n.assert_refused(&scope, n.a.mark_subagents_seen(&["s_2".into()]).await);
    n.assert_refused(&scope, n.a.claim_subagent_deliveries(&session("x")).await);
    n.assert_refused(&scope, n.a.reown_running_subagents(&"x".to_string(), i64::MAX as u64).await);
    n.assert_refused(
        &scope,
        n.a.stop_session_subagents(&session("x"), SubagentStopReason::ConversationDeleted, now).await,
    );
    assert!(n.b.get_subagent("s_4").await.unwrap().is_none());
    assert_eq!(n.b.get_subagent("s_1").await.unwrap().unwrap().state, SubagentState::Running);

    // The same writes by the new owner succeed.
    n.b.open_subagents(vec![worker("s_4")]).await.unwrap();
    assert!(n.b.claim_subagent_deliveries(&session("x")).await.unwrap().len() == 1, "s_2's question");
    n.b.reply_subagent("s_2", now, now).await.unwrap().unwrap();
    n.b.ask_subagent("s_2", "again?", 0, now).await.unwrap().unwrap();
    assert_eq!(n.b.mark_subagents_seen(&["s_2".into()]).await.unwrap(), vec!["s_2".to_string()]);
    n.b.settle_subagent("s_1", answered(), now).await.unwrap().unwrap();
    assert!(matches!(
        n.b.stop_subagent("s_3", SubagentStopReason::ByAgent, now).await.unwrap(),
        StopOutcome::Stopped(_)
    ));
    assert_eq!(n.b.reown_running_subagents(&"x".to_string(), i64::MAX as u64).await.unwrap().len(), 1);
    assert_eq!(
        n.b.stop_session_subagents(&session("x"), SubagentStopReason::ConversationDeleted, now)
            .await
            .unwrap()
            .len(),
        2,
        "s_2 asking and s_4 running"
    );
}

/// Two claimers on two connections race for the same rows: each script is taken once.
#[tokio::test]
async fn detached_claims_race_and_split_the_rows() {
    use crate::schema::DetachedScriptState;
    let n = nodes();
    for i in 0..20 {
        let id = format!("d_{i}");
        n.a.open_detached_script(detached(&id, None)).await.unwrap();
        assert!(
            n.a.close_detached_script(
                &id,
                DetachedScriptState::Succeeded,
                serde_json::json!({}),
                serde_json::json!([]),
                crate::time::now(),
            )
            .await
            .unwrap()
        );
    }
    let (a, b) = (n.a.clone(), n.b.clone());
    let ta = tokio::spawn(async move { a.claim_deliverable_scripts(&session("x")).await.unwrap() });
    let tb = tokio::spawn(async move { b.claim_deliverable_scripts(&session("x")).await.unwrap() });
    let mut ids: Vec<String> = ta.await.unwrap().into_iter().chain(tb.await.unwrap()).map(|s| s.id).collect();
    assert_eq!(ids.len(), 20);
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 20, "a script was claimed twice");
}

/// Cancel and finish race on two connections: exactly one of them wins.
#[tokio::test]
async fn detached_cancel_and_close_race() {
    use crate::schema::{Canceller, DetachedScriptState};
    let n = nodes();
    for round in 0..10 {
        let id = format!("d_{round}");
        n.a.open_detached_script(detached(&id, None)).await.unwrap();
        let (a, b, ia, ib) = (n.a.clone(), n.b.clone(), id.clone(), id.clone());
        let close = tokio::spawn(async move {
            a.close_detached_script(&ia, DetachedScriptState::Succeeded, serde_json::json!({}), serde_json::json!([]), crate::time::now())
                .await
                .unwrap()
        });
        let cancel = tokio::spawn(async move {
            b.cancel_detached_script(&ib, Canceller::Agent("x".into()), crate::time::now()).await.unwrap()
        });
        let closed = close.await.unwrap();
        let cancelled = matches!(cancel.await.unwrap(), CancelOutcome::Cancelled(_));
        assert!(closed ^ cancelled, "round {round}: closed {closed}, cancelled {cancelled}");
    }
}

#[tokio::test]
async fn stale_task_run_write_refused() {
    let n = nodes();
    let scope = Scope::Control;
    n.a_then_b(&scope).await;
    let now = crate::time::now();
    let run = n
        .a
        .open_task_run("x".into(), "daily".into(), now, "key".into())
        .await
        .unwrap();
    n.b_takes_over(&scope).await;

    n.assert_refused(
        &scope,
        n.a.open_task_run("x".into(), "other".into(), now, "key2".into())
            .await,
    );
    n.assert_refused(&scope, n.a.close_task_run(run, TaskRunState::Answered, now).await);
    let open = n.b.open_task_runs().await.unwrap();
    assert_eq!(open.len(), 1, "the run stays open, the second was never written");

    n.b.close_task_run(run, TaskRunState::Answered, now)
        .await
        .unwrap();
    n.b.open_task_run("x".into(), "other".into(), now, "key2".into())
        .await
        .unwrap();
}

/// Rows opened by an owner carry its fence (`owner_node`, `owner_token`); a process that holds no
/// fence leaves them null.
#[tokio::test]
async fn opened_rows_carry_the_owner() {
    let n = nodes();
    let scope = Scope::Agent("x".into());
    let fence = n.a.claim(&scope, "node-a", None).await.unwrap();
    n.a.open_client_call("c1", &session("x")).await.unwrap();
    n.b.open_client_call("c2", &session("x")).await.unwrap_or(());
    let owner = |uid: &str| {
        scalar(
            &n.a,
            "SELECT owner_node || ':' || CAST(owner_token AS TEXT) FROM client_tool_call WHERE history_uid = ?1",
            uid,
        )
    };
    assert_eq!(owner("c1"), Some(format!("node-a:{}", fence.token)));
    // B held no fence yet: its row is unowned (NULL concatenates to NULL).
    assert_eq!(owner("c2"), Some(String::new()));
}

/// A gateway holds no fence and is never refused (contract rule 5).
#[tokio::test]
async fn unfenced_writer_unaffected() {
    let n = nodes();
    let scope = Scope::Agent("x".into());
    n.a_then_b(&scope).await;
    n.b_takes_over(&scope).await;
    // A third node over the same database that never claimed anything.
    let (gateway, _dir) = node_storage(n.b.conn.clone());
    gateway
        .save_session_history(session("x"), request("from a gateway"))
        .await
        .unwrap();
    let id = gateway
        .accept_turn(&session("x"), &VizierRequest::default())
        .await
        .unwrap();
    assert!(id > 0);
}

/// PostgreSQL only: a claim waits for a fenced write in progress (research R2).
#[test]
fn claim_waits_for_inflight_write() {
    use std::sync::atomic::{AtomicBool, Ordering};
    if !crate::tests_support::testing_postgres() {
        return;
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let n = nodes();
    let scope = Scope::Agent("x".into());
    rt.block_on(n.a_then_b(&scope));

    // A's write is in progress: it has passed its fence check and holds the share lock.
    let conn = n.a.conn.lock().unwrap();
    let tx = conn.unchecked_transaction().unwrap();
    super::fence::check_fence(&tx, &n.a.fences, &scope).unwrap();

    let claimed = Arc::new(AtomicBool::new(false));
    let handle = {
        let (b, scope, claimed) = (n.b.clone(), scope.clone(), claimed.clone());
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(b.claim(&scope, "b", None)).unwrap();
            claimed.store(true, Ordering::SeqCst);
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        !claimed.load(Ordering::SeqCst),
        "the claim must wait for the write in progress"
    );
    tx.commit().unwrap();
    drop(conn);
    handle.join().unwrap();
    assert!(claimed.load(Ordering::SeqCst));
    // After the claim, A's next write is refused.
    let late = rt.block_on(n.a.save_session_history(session("x"), request("late")));
    assert!(is_ownership_lost(&late.unwrap_err()));
}

crate::postgres_tests!(
    stale_history_save_refused,
    stale_turn_transition_refused,
    stale_client_call_write_refused,
    stale_sensitive_ask_write_refused,
    stale_detached_script_write_refused,
    stale_subagent_write_refused,
    detached_claims_race_and_split_the_rows,
    detached_cancel_and_close_race,
    stale_task_run_write_refused,
    opened_rows_carry_the_owner,
    unfenced_writer_unaffected,
    claim_waits_for_inflight_write,
);
