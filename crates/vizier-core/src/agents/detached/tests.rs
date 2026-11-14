//! Unit tests for detached scripts (spec 058 T024): launch refusals, the time limit, the
//! exactly-once claim, cancel and its cascade, a task stopping when its row is cancelled
//! elsewhere, and redaction of what is stored.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use super::*;
use crate::{
    agents::tools::{ToolRouter, VizierTool, VizierToolSet},
    sandbox::SandboxLimits,
    schema::{VizierChannelId, VizierSession},
    storage::sqlite::SqlStorage,
};

struct Fixture {
    scripts: DetachedScripts,
    storage: Arc<VizierStorage>,
    _dir: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    SqlStorage::init_schema(&conn).unwrap();
    let storage = Arc::new(VizierStorage::new(SqlStorage::new(
        crate::storage::sql::SqlStore::sqlite(conn),
        Arc::new(crate::storage::document::BlobDocumentStore::local(dir.path().to_path_buf())),
    )));
    let scripts = DetachedScripts::new(
        storage.clone(),
        VizierTransport::new(),
        Notifications::new(storage.clone()),
    );
    Fixture { scripts, storage, _dir: dir }
}

fn chat() -> VizierSession {
    VizierSession(
        "agent-1".into(),
        VizierChannelId::HTTP("dani".into(), "chat".into()),
        Some("t".into()),
    )
}

fn ctx(session: VizierSession, depth: u8) -> ToolContext {
    ToolContext {
        loaded_images: Default::default(),
        client: None,
        questions: crate::questions::QuestionsHandle::new(Default::default()),
        interactive: true,
        session,
        pending_attachments: Default::default(),
        hooks: None,
        background_depth: depth,
        subagent_depth: 0,
        deadline: None,
        dream_sources: vec![],
        secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
        project: None,
        detached: false,
        parent_detached_id: None,
    }
}

fn launch(code: &str, timeout_secs: u64) -> Launch {
    Launch { intent: "test the thing".into(), code: code.into(), timeout_secs }
}

fn report(ok: bool, result: serde_json::Value) -> ExecutionReport {
    ExecutionReport {
        ok,
        result,
        stdout: String::new(),
        error: None,
        tool_calls: vec![],
        duration_ms: 1,
    }
}

/// A runner that answers at once.
fn answering(value: serde_json::Value) -> ScriptRunner {
    Box::new(move |_ctx, _progress| Box::pin(async move { (report(true, value), vec![]) }))
}

/// A runner that never answers and says when it is dropped.
fn hanging(dropped: Arc<AtomicBool>) -> ScriptRunner {
    struct Flag(Arc<AtomicBool>);
    impl Drop for Flag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    Box::new(move |_ctx, progress| {
        // Outside the async block: a run cancelled before its first poll still drops it.
        let flag = Flag(dropped);
        Box::pin(async move {
            let _flag = flag;
            progress(&[ToolInvocationRecord {
                seq: 1,
                name: "shell_exec".into(),
                arguments: serde_json::json!({"command": "sleep 60"}),
                ok: true,
                error: None,
                duration_ms: 5,
                ends_turn: false,
            }]);
            std::future::pending::<()>().await;
            unreachable!()
        })
    })
}

async fn until_final(storage: &VizierStorage, id: &str) -> DetachedScript {
    for _ in 0..300 {
        let script = storage.get_detached_script(id).await.unwrap().unwrap();
        if script.state.is_final() {
            return script;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("{id} never finished");
}

async fn until(mut check: impl FnMut() -> bool) {
    for _ in 0..300 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("condition never held");
}

#[tokio::test]
async fn launches_nobody_waits_on_and_too_deep_ones_are_refused() {
    let f = fixture();
    let task = VizierSession(
        "agent-1".into(),
        VizierChannelId::Task("tick".into(), crate::time::now()),
        None,
    );
    let err = f.scripts.launch(&ctx(task, 0), launch("1", 60), answering(1.into())).await.unwrap_err();
    assert!(err.0.starts_with("cannot detach here: nobody is waiting to be told"), "{err}");

    let dream = VizierSession(
        "agent-1".into(),
        VizierChannelId::Dream(Box::new(chat()), crate::schema::DreamStage::Extraction),
        None,
    );
    let err = f.scripts.launch(&ctx(dream, 0), launch("1", 60), answering(1.into())).await.unwrap_err();
    assert!(err.0.starts_with("cannot detach here"), "{err}");

    let err = f
        .scripts
        .launch(&ctx(chat(), MAX_BACKGROUND_DEPTH), launch("1", 60), answering(1.into()))
        .await
        .unwrap_err();
    assert!(err.0.starts_with("nesting limit (3) reached"), "{err}");

    assert!(f.storage.list_detached_scripts(&chat(), 50).await.unwrap().is_empty());
}

#[test]
fn a_detached_timeout_is_checked_and_an_inline_one_ignored() {
    assert_eq!(validate_timeout(false, None).unwrap(), DEFAULT_TIMEOUT_SECS);
    assert_eq!(validate_timeout(false, Some(0)).unwrap(), DEFAULT_TIMEOUT_SECS);
    assert_eq!(validate_timeout(true, None).unwrap(), DEFAULT_TIMEOUT_SECS);
    assert_eq!(validate_timeout(true, Some(5)).unwrap(), 5);
    for secs in [0, 3601] {
        let err = validate_timeout(true, Some(secs)).unwrap_err();
        assert!(err.0.starts_with("timeout_secs must be between 1 and 3600"), "{err}");
    }
}

#[tokio::test]
async fn a_finished_script_is_claimed_exactly_once() {
    let f = fixture();
    let script = f.scripts.launch(&ctx(chat(), 0), launch("1", 60), answering("done".into())).await.unwrap();
    assert_eq!(script.state, DetachedScriptState::Running);
    assert!(script.id.starts_with("d_"));

    let stored = until_final(&f.storage, &script.id).await;
    assert_eq!(stored.state, DetachedScriptState::Succeeded);
    assert_eq!(stored.outcome.as_ref().unwrap()["result"], "done");
    assert!(stored.outcome.as_ref().unwrap().get("tool_calls").is_none());

    let session = chat();
    let (a, b) = tokio::join!(f.scripts.claim(&session), f.scripts.claim(&session));
    assert_eq!(a.len() + b.len(), 1, "claimed {} + {}", a.len(), b.len());
    assert!(f.scripts.claim(&chat()).await.is_empty());
    until(|| f.scripts.live_count() == 0).await;
}

#[tokio::test]
async fn the_time_limit_stops_the_script() {
    let f = fixture();
    let dropped = Arc::new(AtomicBool::new(false));
    let script = f.scripts.launch(&ctx(chat(), 0), launch("1", 1), hanging(dropped.clone())).await.unwrap();
    let stored = until_final(&f.storage, &script.id).await;
    assert_eq!(stored.state, DetachedScriptState::TimedOut);
    assert_eq!(stored.outcome, Some(serde_json::json!("the script's time limit (1s) was reached")));
    assert!(dropped.load(Ordering::SeqCst), "the run was not stopped");
    assert_eq!(f.scripts.claim(&chat()).await.len(), 1, "a timed-out script is delivered");
}

#[tokio::test]
async fn cancel_stops_the_task_and_nothing_is_delivered() {
    let f = fixture();
    let dropped = Arc::new(AtomicBool::new(false));
    let script = f.scripts.launch(&ctx(chat(), 0), launch("1", 60), hanging(dropped.clone())).await.unwrap();
    // The progress write records the call it made.
    let storage = f.storage.clone();
    let id = script.id.clone();
    for _ in 0..100 {
        if storage.get_detached_script(&id).await.unwrap().unwrap().tool_call_count() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let outcome = f.scripts.cancel(&script.id, Canceller::Agent("agent-1".into())).await.unwrap();
    assert_eq!(outcome, CancelOutcome::Cancelled(vec![script.id.clone()]));
    until(|| dropped.load(Ordering::SeqCst)).await;
    until(|| f.scripts.live_count() == 0).await;

    let stored = f.storage.get_detached_script(&script.id).await.unwrap().unwrap();
    assert_eq!(stored.state, DetachedScriptState::Cancelled);
    assert_eq!(stored.tool_call_count(), 1);
    assert!(f.scripts.claim(&chat()).await.is_empty());
    assert_eq!(
        f.scripts.cancel(&script.id, Canceller::Agent("agent-1".into())).await.unwrap(),
        CancelOutcome::AlreadyFinished(DetachedScriptState::Cancelled)
    );
}

#[tokio::test]
async fn cancel_cascades_to_a_script_launched_beneath_it() {
    let f = fixture();
    let (outer_dropped, inner_dropped) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
    let outer = f.scripts.launch(&ctx(chat(), 0), launch("1", 60), hanging(outer_dropped.clone())).await.unwrap();
    // A sub-agent turn of the outer script detaches its own.
    let mut sub = ctx(VizierSession("agent-2".into(), VizierChannelId::Subagent, Some("p".into())), 1);
    sub.parent_detached_id = Some(outer.id.clone());
    let inner = f.scripts.launch(&sub, launch("1", 60), hanging(inner_dropped.clone())).await.unwrap();
    assert_eq!(inner.parent_id.as_deref(), Some(outer.id.as_str()));

    let outcome = f.scripts.cancel(&outer.id, Canceller::Agent("agent-1".into())).await.unwrap();
    assert_eq!(outcome, CancelOutcome::Cancelled(vec![outer.id.clone(), inner.id.clone()]));
    until(|| outer_dropped.load(Ordering::SeqCst) && inner_dropped.load(Ordering::SeqCst)).await;
}

/// Another process cancelled the row: the task sees it at its next progress write and stops.
#[tokio::test]
async fn a_task_stops_when_its_row_reads_cancelled() {
    let f = fixture();
    let dropped = Arc::new(AtomicBool::new(false));
    let calls: Arc<Mutex<Option<ProgressFn>>> = Default::default();
    let runner: ScriptRunner = {
        let (dropped, calls) = (dropped.clone(), calls.clone());
        Box::new(move |_ctx, progress| {
            *calls.lock() = Some(progress);
            hanging(dropped)(ctx(chat(), 0), Arc::new(|_: &[ToolInvocationRecord]| {}))
        })
    };
    let script = f.scripts.launch(&ctx(chat(), 0), launch("1", 60), runner).await.unwrap();
    until(|| calls.lock().is_some()).await;

    f.storage
        .cancel_detached_script(&script.id, Canceller::Agent("agent-1".into()), crate::time::now())
        .await
        .unwrap();
    let progress = calls.lock().clone().unwrap();
    progress(&[]);
    until(|| dropped.load(Ordering::SeqCst)).await;
    until(|| f.scripts.live_count() == 0).await;
}

#[tokio::test]
async fn interrupting_the_agent_stops_its_scripts_and_makes_them_deliverable() {
    let f = fixture();
    let dropped = Arc::new(AtomicBool::new(false));
    let script = f.scripts.launch(&ctx(chat(), 0), launch("1", 60), hanging(dropped.clone())).await.unwrap();
    f.scripts.interrupt_agent("agent-1").await;
    until(|| dropped.load(Ordering::SeqCst)).await;
    let stored = f.storage.get_detached_script(&script.id).await.unwrap().unwrap();
    assert_eq!(stored.state, DetachedScriptState::Interrupted);
    assert_eq!(f.scripts.claim(&chat()).await.len(), 1);
}

#[tokio::test]
async fn deleting_the_agent_stops_its_scripts_and_leaves_no_row() {
    let f = fixture();
    let dropped = Arc::new(AtomicBool::new(false));
    let script = f.scripts.launch(&ctx(chat(), 0), launch("1", 60), hanging(dropped.clone())).await.unwrap();
    f.scripts.delete_agent("agent-1").await;
    until(|| dropped.load(Ordering::SeqCst)).await;
    assert!(f.storage.get_detached_script(&script.id).await.unwrap().is_none());
}

#[test]
fn stored_calls_cut_argument_previews() {
    let long = "x".repeat(2_000);
    let stored = stored_calls(&[ToolInvocationRecord {
        seq: 1,
        name: "write_tmp".into(),
        arguments: serde_json::json!({"name": "a", "content": long, "list": vec![1; 1_000]}),
        ok: true,
        error: None,
        duration_ms: 1,
        ends_turn: false,
    }]);
    let args = &stored[0]["arguments"];
    assert_eq!(args["name"], "a");
    assert_eq!(args["content"].as_str().unwrap().chars().count(), MAX_ARGUMENT_PREVIEW_CHARS + 1);
    assert!(args["list"].is_string());
}

#[test]
fn a_huge_outcome_is_cut_to_fit() {
    let mut big = report(true, serde_json::json!("y".repeat(100_000)));
    big.stdout = "z".repeat(100_000);
    let outcome = bounded_outcome(&big, &["a.png".into()]);
    assert!(outcome.to_string().len() <= MAX_OUTCOME_BYTES, "{}", outcome.to_string().len());
    assert!(outcome["result"].as_str().unwrap().ends_with(delivery::CUT_MARKER));
    assert_eq!(outcome["files"][0], "a.png");
}

/// Through the real `execute_python`: the handle comes back at once, and a value the turn
/// armed as secret is scrubbed from the stored intent, code and outcome (FR-006).
#[tokio::test]
async fn execute_python_detaches_and_stores_nothing_secret() {
    const TOKEN: &str = "ghp_detached-secret-value-42";
    let f = fixture();
    let router = ToolRouter {
        default_toolset: VizierToolSet::new(),
        user_toolset: VizierToolSet::new(),
        integration_toolset: VizierToolSet::new(),
        addons: Default::default(),
        mcp: Default::default(),
        reserved: Arc::new(vec![]),
    };
    let tool = crate::agents::tools::python_for_tests(
        router,
        SandboxLimits { timeout: Duration::from_secs(5), tools_enabled: true },
        Some(f.scripts.clone()),
    );
    let ctx = ctx(chat(), 0);
    ctx.secret_redactor.arm(TOKEN);

    let started = std::time::Instant::now();
    let reply = tool
        .call(
            serde_json::from_value(serde_json::json!({
                "intent": format!("use {TOKEN}"),
                "code": format!("'{TOKEN}'"),
                "detached": true,
                "timeout_secs": 30,
            }))
            .unwrap(),
            &ctx,
        )
        .await
        .unwrap();
    assert!(started.elapsed() < Duration::from_secs(1));
    let crate::schema::VizierResponseContent::ToolResponse { response } = reply.content else {
        panic!("not a tool response");
    };
    assert_eq!(response["detached"], true);
    assert_eq!(response["state"], "running");
    assert_eq!(response["timeout_secs"], 30);
    let id = response["id"].as_str().unwrap().to_string();

    let stored = until_final(&f.storage, &id).await;
    assert_eq!(stored.state, DetachedScriptState::Succeeded);
    let everything = serde_json::to_string(&stored).unwrap();
    assert!(!everything.contains(TOKEN), "{everything}");
    assert!(everything.contains("[redacted]"));

    // An out-of-range limit on a detached call is refused; an inline call ignores the field.
    let err = tool
        .call(
            serde_json::from_value(serde_json::json!({"intent": "x", "code": "1", "detached": true, "timeout_secs": 0})).unwrap(),
            &ctx,
        )
        .await
        .unwrap_err();
    assert!(err.0.contains("between 1 and 3600"), "{err}");
    assert!(
        tool.call(
            serde_json::from_value(serde_json::json!({"intent": "x", "code": "1", "timeout_secs": 0})).unwrap(),
            &ctx,
        )
        .await
        .is_ok()
    );
}
