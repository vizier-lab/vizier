use std::{fs, path::PathBuf};

use anyhow::{Result, bail};
use clap::{Args, ValueEnum};
use daemonize::Daemonize;

use super::server::run_server;
use vizier_core::{
    config::{
        RunOverrides,
        storage::{ObjectsConfig, PostgresConfig, S3Config, StorageConfig},
    },
    server_role::{ServerRole, ServerRoleSet},
};

/// `filesystem` is intentionally not a valid value here (research.md §10, FR-025). An operator
/// still passing `--storage filesystem` gets clap's own "invalid value" error rather than
/// silently falling back or crashing deep in startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "lowercase")]
pub(super) enum StorageKind {
    Sqlite,
    Postgres,
}

impl StorageKind {
    /// The backend to use: the config's own settings when it already names this kind (so a
    /// YAML `pool_size` or `url_env` survives `--storage postgres`), defaults otherwise.
    pub(super) fn resolve(self, current: &StorageConfig) -> StorageConfig {
        match (self, current) {
            (StorageKind::Postgres, StorageConfig::Postgres(_)) => current.clone(),
            (StorageKind::Postgres, _) => StorageConfig::Postgres(PostgresConfig::default()),
            (StorageKind::Sqlite, _) => StorageConfig::Sqlite,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "lowercase")]
pub(super) enum ObjectsKind {
    Local,
    S3,
}

impl ObjectsKind {
    pub(super) fn resolve(self, current: &ObjectsConfig) -> ObjectsConfig {
        match (self, current) {
            (ObjectsKind::S3, ObjectsConfig::S3(_)) => current.clone(),
            (ObjectsKind::S3, _) => ObjectsConfig::S3(S3Config::default()),
            (ObjectsKind::Local, _) => ObjectsConfig::Local,
        }
    }
}

/// Parse `VIZIER_<NAME>` as a clap value for runs outside Docker (the entrypoint maps the same
/// variables to flags). A present invalid value is an error, never a silent fallback to the
/// embedded backend.
pub(super) fn env_value<T: ValueEnum>(name: &str) -> Result<Option<T>> {
    let Some(value) = std::env::var_os(name) else {
        return Ok(None);
    };
    let value = value
        .into_string()
        .map_err(|_| anyhow::anyhow!("{name} is not valid UTF-8"))?;
    T::from_str(value.trim(), true)
        .map(Some)
        .map_err(|_| anyhow::anyhow!("{name} has an invalid value"))
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

    #[arg(
        long,
        value_name = "PORT",
        help = "HTTP server port (overrides config)"
    )]
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
        help = "database backend: sqlite (default, embedded) or postgres (URL from VIZIER_DATABASE_URL); env VIZIER_STORAGE"
    )]
    storage: Option<StorageKind>,

    #[arg(
        long,
        value_name = "OBJECTS",
        help = "file backend: local (default, the workspace) or s3 (VIZIER_S3_* variables); env VIZIER_OBJECTS"
    )]
    objects: Option<ObjectsKind>,

    #[arg(long, value_name = "N", help = "tokio worker thread count")]
    workers: Option<usize>,

    #[arg(long, value_name = "SECS", help = "websocket idle timeout in seconds")]
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

    #[arg(
        long,
        value_name = "ROLE[,ROLE]",
        value_parser = parse_server_role_set,
        help = "server roles to run: all (default), or a comma list of gateway-http, gateway-grpc, worker, channels, control; env VIZIER_ROLE"
    )]
    role: Option<ServerRoleSet>,
}

/// `--role` as a [`ServerRoleSet`]: `all`, one role, or a comma list.
fn parse_server_role_set(raw: &str) -> Result<ServerRoleSet, String> {
    raw.parse()
}

/// The roles this binary carries, from its `role-*` features. A role left out of the build is
/// refused with the flag to rebuild with, rather than starting a process that silently does less.
fn compiled_roles() -> ServerRoleSet {
    let mut roles = ServerRoleSet::none();
    #[cfg(feature = "role-gateway-http")]
    roles.insert(ServerRole::GatewayHttp);
    #[cfg(feature = "role-gateway-grpc")]
    roles.insert(ServerRole::GatewayGrpc);
    #[cfg(feature = "role-worker")]
    roles.insert(ServerRole::Worker);
    #[cfg(feature = "role-channels")]
    roles.insert(ServerRole::Channels);
    #[cfg(feature = "role-control")]
    roles.insert(ServerRole::Control);
    roles
}

/// Refuses what this process cannot be: a role the binary was not built with, or one whose
/// sub-issue has not landed yet (#166–#170). `all` is today's single node and is accepted as
/// long as the build carries every role.
fn check_roles(requested: ServerRoleSet, compiled: ServerRoleSet) -> Result<ServerRoleSet> {
    if let Some(missing) = requested.iter().find(|role| !compiled.contains(*role)) {
        bail!(
            "this build has no {missing} role: rebuild with --no-default-features --features {}",
            missing.feature()
        );
    }
    if requested.is_all() {
        return Ok(requested);
    }
    for role in requested.iter() {
        if let Some(issue) = vizier_fleet::pending(role) {
            bail!("the {role} role is not implemented yet ({issue}); run with --role all");
        }
    }
    Ok(requested)
}

/// `--role`, else `VIZIER_ROLE`, else every role. A present but invalid `VIZIER_ROLE` is an
/// error, never a silent fallback to the single-node default.
fn resolve_roles(flag: Option<ServerRoleSet>) -> Result<ServerRoleSet> {
    let requested = match flag {
        Some(roles) => roles,
        None => match std::env::var("VIZIER_ROLE") {
            Ok(raw) if !raw.trim().is_empty() => raw
                .trim()
                .parse::<ServerRoleSet>()
                .map_err(|e| anyhow::anyhow!("VIZIER_ROLE: {e}"))?,
            _ => ServerRoleSet::all(),
        },
    };
    check_roles(requested, compiled_roles())
}

pub fn run(args: RunArgs) -> Result<()> {
    super::logo::print_logo();

    let mut config = super::config_resolve::resolve(args.config.clone())?;

    let workspace_override = args.data_dir.clone().or(args.workspace.clone());
    let storage = match args.storage {
        Some(kind) => Some(kind),
        None => env_value("VIZIER_STORAGE")?,
    };
    let objects = match args.objects {
        Some(kind) => Some(kind),
        None => env_value("VIZIER_OBJECTS")?,
    };
    let overrides = RunOverrides {
        workspace: workspace_override,
        port: args.port,
        storage: storage.map(|kind| kind.resolve(&config.storage)),
        objects: objects.map(|kind| kind.resolve(&config.objects)),
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

    // Before anything is started: a role this process cannot be is a startup error, not a
    // server that comes up doing less than it was asked to.
    let roles = resolve_roles(args.role)?;
    tracing::info!(roles = %roles, "starting vizier");

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

    if let Err(err) = run_server(config, roles) {
        tracing::error!("server exited with error: {}", err);
        return Err(err);
    }

    Ok(())
}

#[cfg(test)]
mod role_tests {
    use super::*;

    #[test]
    fn all_is_one_process_doing_everything_and_needs_no_sub_issue() {
        let roles = check_roles(ServerRoleSet::all(), ServerRoleSet::all()).unwrap();
        assert!(roles.is_all());
    }

    #[test]
    fn a_selected_role_is_refused_with_its_sub_issue() {
        let error = check_roles(ServerRoleSet::only(ServerRole::Worker), ServerRoleSet::all())
            .unwrap_err()
            .to_string();
        assert!(error.contains("worker"), "{error}");
        assert!(error.contains("issues/168"), "{error}");
        assert!(error.contains("--role all"), "{error}");
    }

    #[test]
    fn a_role_the_build_lacks_says_how_to_rebuild() {
        let error = check_roles(
            ServerRoleSet::only(ServerRole::Channels),
            ServerRoleSet::only(ServerRole::Worker),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("role-channels"), "{error}");
    }

    #[test]
    fn a_build_with_role_all_carries_every_role() {
        if cfg!(feature = "role-all") {
            assert!(compiled_roles().is_all());
        }
    }

    #[test]
    fn the_role_flag_takes_all_a_role_or_a_list() {
        assert_eq!(parse_server_role_set("all").unwrap(), ServerRoleSet::all());
        assert_eq!(
            parse_server_role_set("worker").unwrap(),
            ServerRoleSet::only(ServerRole::Worker)
        );
        assert_eq!(
            parse_server_role_set("worker, control").unwrap(),
            ServerRoleSet::only(ServerRole::Worker).with(ServerRole::Control)
        );
        assert!(parse_server_role_set("gateway").is_err());
    }
}
