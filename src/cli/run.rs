use std::{fs, path::PathBuf};

use anyhow::Result;
use clap::{Args, ValueEnum};
use daemonize::Daemonize;

use super::server::run_server;
use crate::config::{RunOverrides, storage::StorageConfig};

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

pub fn run(args: RunArgs) -> Result<()> {
    super::logo::print_logo();

    let mut config = super::config_resolve::resolve(args.config.clone())?;

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
        return Err(err);
    }

    Ok(())
}
