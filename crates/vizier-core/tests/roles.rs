//! Fleet role plumbing (vizier-lab/vizier#161): `VizierRuntime` spawns only the subsystems the
//! selected roles own, and the scheduler and the channel readers ask the [`Gate`] before they
//! do anything.
//!
//! `--role all` is today's server, so the first test pins it: the same four subsystems, in the
//! same order. The others pin the plumbing the roles hang off.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use vizier_core::{
    config::VizierConfig,
    runtime::{Gate, HealthReport, RuntimeOptions, SubsystemState, VizierRuntime},
    server_role::{ServerRole, ServerRoleSet},
};

/// A gate that counts how often it is asked, and answers as told.
struct CountingGate {
    asks: Arc<AtomicUsize>,
    answer: bool,
}

impl Gate for CountingGate {
    fn may_run(&self) -> bool {
        self.asks.fetch_add(1, Ordering::SeqCst);
        self.answer
    }
}

/// A runtime on a fresh workspace, closed on drop of the returned directory's owner.
async fn start(
    dir: &tempfile::TempDir,
    roles: ServerRoleSet,
    gate: Option<Arc<dyn Gate>>,
) -> VizierRuntime {
    VizierRuntime::start(RuntimeOptions {
        config: VizierConfig::with_workspace(dir.path().to_path_buf()),
        roles,
        gate,
    })
    .await
    .unwrap()
}

fn running(health: &HealthReport) -> Vec<&'static str> {
    health
        .subsystems
        .iter()
        .filter(|(_, state)| matches!(state, SubsystemState::Running))
        .map(|(name, _)| *name)
        .collect()
}

/// The gate is asked from inside a spawned task, so give the runtime a moment to poll it.
async fn asks(asks: &Arc<AtomicUsize>) -> usize {
    for _ in 0..100 {
        if asks.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    asks.load(Ordering::SeqCst)
}

#[tokio::test(flavor = "multi_thread")]
async fn the_default_is_one_node_doing_everything() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = start(&dir, ServerRoleSet::default(), None).await;

    assert!(runtime.roles().is_all());
    let health = runtime.health().await;
    assert!(health.roles.is_all());
    assert_eq!(
        running(&health),
        ["agent manager", "scheduler", "channel readers", "command handler"],
        "the default runtime is the four subsystems of #51, in order"
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_worker_only_runtime_runs_the_agent_manager_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = start(&dir, ServerRoleSet::only(ServerRole::Worker), None).await;

    let health = runtime.health().await;
    assert_eq!(health.roles, ServerRoleSet::only(ServerRole::Worker));
    assert_eq!(running(&health), ["agent manager", "command handler"]);
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_control_only_runtime_runs_the_scheduler_and_asks_the_gate() {
    let asks_count = Arc::new(AtomicUsize::new(0));
    let gate = CountingGate {
        asks: asks_count.clone(),
        answer: false,
    };
    let dir = tempfile::tempdir().unwrap();
    let runtime = start(
        &dir,
        ServerRoleSet::only(ServerRole::Control),
        Some(Arc::new(gate)),
    )
    .await;

    assert_eq!(running(&runtime.health().await), ["scheduler", "command handler"]);
    assert!(
        asks(&asks_count).await > 0,
        "the scheduler asks the gate before doing control work"
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_channels_only_runtime_runs_the_readers_and_asks_the_gate() {
    let asks_count = Arc::new(AtomicUsize::new(0));
    let gate = CountingGate {
        asks: asks_count.clone(),
        answer: false,
    };
    let dir = tempfile::tempdir().unwrap();
    let runtime = start(
        &dir,
        ServerRoleSet::only(ServerRole::Channels),
        Some(Arc::new(gate)),
    )
    .await;

    assert_eq!(
        running(&runtime.health().await),
        ["channel readers", "command handler"]
    );
    assert!(
        asks(&asks_count).await > 0,
        "the readers ask the gate before running any reader"
    );
    runtime.shutdown().await.unwrap();
}
