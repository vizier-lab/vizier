use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::Result;
use clap::{Args, ValueEnum};
use daemonize::Daemonize;
use tokio::task::JoinSet;

use crate::{
    agents::VizierAgents, channels::VizierChannels, command::VizierCommandServer,
    config::{
        storage::StorageConfig,
        RunOverrides, VizierConfig,
    },
    dependencies::VizierDependencies,
    scheduler::VizierScheduler,
};

/// `filesystem` is intentionally not a valid value here (research.md §10, FR-025) — sqlite is
/// the sole runtime backend now. An operator still passing `--storage filesystem` gets clap's
/// own "invalid value" error rather than silently falling back or crashing deep in startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "lowercase")]
enum StorageKind {
    Sqlite,
}

impl From<StorageKind> for StorageConfig {
    fn from(kind: StorageKind) -> Self {
        match kind {
            StorageKind::Sqlite => StorageConfig::Sqlite,
        }
    }
}

#[derive(Debug, Args, Clone)]
pub struct RunArgs {
    #[arg(
        short,
        long,
        value_name = "PATH",
        value_hint = clap::ValueHint::DirPath,
        help = "path to .vizier.yaml or .vizier/config.yaml (optional, uses defaults if omitted)"
    )]
    config: Option<std::path::PathBuf>,

    #[arg(
        short = 'd',
        long = "detached",
        help = "run the server detached (daemonize) in the background"
    )]
    detached: bool,

    #[arg(long, value_name = "PORT", help = "HTTP server port (overrides config)")]
    port: Option<u32>,

    #[arg(
        long,
        value_name = "PATH",
        value_hint = clap::ValueHint::DirPath,
        help = "workspace directory (overrides config and VIZIER_DATA_DIR)"
    )]
    workspace: Option<PathBuf>,

    #[arg(
        long,
        value_name = "PATH",
        value_hint = clap::ValueHint::DirPath,
        help = "alias of --workspace, friendlier name; wins over --workspace if both are set"
    )]
    data_dir: Option<PathBuf>,

    #[arg(
        long,
        value_name = "STORAGE",
        help = "storage backend (sqlite is the only supported value; kept as a flag for forwards compatibility)"
    )]
    storage: Option<StorageKind>,

    #[arg(long, value_name = "N", help = "tokio worker thread count")]
    workers: Option<usize>,

    #[arg(
        long,
        value_name = "SECS",
        help = "websocket idle timeout in seconds"
    )]
    ws_idle_timeout: Option<u64>,

    #[arg(
        long,
        value_name = "BOOL",
        help = "refresh the model catalogue from models.dev when people use it (default true)"
    )]
    catalogue_refresh: Option<bool>,

    #[arg(
        long,
        value_name = "DURATION",
        help = "how long a catalogue check stays fresh, e.g. 24h"
    )]
    catalogue_ttl: Option<duration_string::DurationString>,
}

pub fn run_server(config: VizierConfig) -> Result<()> {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.worker_threads(config.worker_threads);

    let runtime = builder.enable_all().build()?;

    runtime.block_on(async move {
        let deps = VizierDependencies::new(config.clone()).await?;
        let exit_transport = deps.clone().transport;
        let exit_signal = exit_transport.exit_signal();

        let mut subsystems = Subsystems::default();

        // Agents first: `VizierAgents::new` registers every agent's request channel, so the
        // scheduler's immediate first tick queues a due task instead of losing it (#26).
        let mut agents = VizierAgents::new(deps.clone()).await?;
        // After the agents re-sent their pending turns, so a recovered job finds them running.
        deps.background_jobs.recover().await;
        subsystems.spawn("agent manager", async move { agents.run().await });

        let mut scheduler = VizierScheduler::new(deps.clone()).await?;
        subsystems.spawn("scheduler", async move { scheduler.run().await });

        let mut channels = VizierChannels::new(config.channels.http.clone(), deps.clone())?;
        subsystems.spawn("http channel", async move { channels.run().await });

        let commands = VizierCommandServer::new(deps.clone())?;
        subsystems.spawn("command server", async move { commands.run().await });

        let transport = deps.clone();
        subsystems.spawn("command handler", async move { transport.run().await });

        tracing::info!("vizier is running!");

        // Every subsystem runs for the life of the process, so one that stops, for whatever
        // reason, leaves vizier half-working. Exit instead, and let whatever runs vizier
        // (Docker's restart policy, systemd) start it again whole (#51).
        tokio::select! {
            _ = exit_signal => {
                subsystems.set.abort_all();
                std::process::exit(0);
            }
            (name, outcome) = subsystems.next_stopped() => {
                tracing::error!("{name} stopped ({outcome}); exiting so vizier can be restarted");
                std::process::exit(1);
            }
        }
    })
}

/// The long-lived tasks the server is made of, by name.
#[derive(Default)]
struct Subsystems {
    set: JoinSet<Result<()>>,
    names: HashMap<tokio::task::Id, &'static str>,
}

impl Subsystems {
    fn spawn(
        &mut self,
        name: &'static str,
        task: impl std::future::Future<Output = Result<()>> + Send + 'static,
    ) {
        let id = self.set.spawn(task).id();
        self.names.insert(id, name);
    }

    /// The first subsystem to stop, and how.
    async fn next_stopped(&mut self) -> (&'static str, String) {
        let (id, outcome) = match self.set.join_next_with_id().await {
            Some(Ok((id, Ok(())))) => (id, "it returned".to_string()),
            Some(Ok((id, Err(e)))) => (id, format!("{e:#}")),
            Some(Err(e)) if e.is_panic() => (e.id(), "it panicked".to_string()),
            Some(Err(e)) => (e.id(), e.to_string()),
            None => return std::future::pending().await,
        };
        (self.names.get(&id).copied().unwrap_or("a subsystem"), outcome)
    }
}

pub fn run(args: RunArgs) -> Result<()> {
    crate::utils::logo::print_logo();

    let mut config = VizierConfig::load(args.config.clone())?;

    let workspace_override = args.data_dir.clone().or(args.workspace.clone());
    let overrides = RunOverrides {
        workspace: workspace_override,
        port: args.port,
        storage: args.storage.map(Into::into),
        workers: args.workers,
        ws_idle_timeout: args.ws_idle_timeout,
        // The env vars work outside Docker too (the entrypoint maps them to these flags).
        catalogue_refresh: args.catalogue_refresh.or_else(|| {
            std::env::var("VIZIER_CATALOGUE_REFRESH")
                .ok()
                .and_then(|v| v.trim().to_ascii_lowercase().parse().ok())
        }),
        catalogue_ttl: args.catalogue_ttl.or_else(|| {
            std::env::var("VIZIER_CATALOGUE_TTL")
                .ok()
                .and_then(|v| v.trim().parse().ok())
        }),
    };
    config.apply_overrides(&overrides);

    let workspace = PathBuf::from(&config.workspace);
    let config = config.clone();

    if args.detached {
        let mut runtime_dir = workspace.clone();
        runtime_dir.push(".runtime");
        runtime_dir.push("logs");
        let _ = fs::create_dir_all(&runtime_dir);

        let mut stdout_path = runtime_dir.clone();
        let now = chrono::Utc::now().to_string();

        stdout_path.push(format!("{}.out", now));
        let stdout = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(stdout_path)?;

        let mut stderr_path = runtime_dir.clone();
        stderr_path.push(format!("{}.err", now));
        let stderr = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(stderr_path)?;

        tracing::info!("vizier will run in the background...");

        let daemonize = Daemonize::new()
            .pid_file("/tmp/vizier.pid")
            .working_directory(workspace.parent().unwrap())
            .umask(0o022)
            .stdout(stdout)
            .stderr(stderr);
        daemonize.start()?;
    }

    if let Err(err) = run_server(config) {
        tracing::error!("server exited with error: {}", err);
    }

    Ok(())
}
