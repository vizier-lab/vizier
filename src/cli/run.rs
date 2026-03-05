use std::{env, fs, path::PathBuf, str::FromStr};

use anyhow::Result;
use clap::Args;

use crate::{
    agent::VizierAgents,
    channels::VizierChannels,
    cli::tui::{self, TuiArgs},
    config::VizierConfig,
    constant::{AGENT_MD, BOOT_MD, IDENT_MD, USER_MD},
    dependencies::VizierDependencies,
    transport::VizierTransport,
};

#[derive(Debug, Args, Clone)]
pub struct RunArgs {
    #[arg(
        short,
        long,
        value_name = "PATH",
        value_hint = clap::ValueHint::DirPath,
        help = "path to .vizier.toml config file",
    )]
    config: Option<std::path::PathBuf>,

    #[arg(long, help = "serve with tui")]
    tui: bool,
}

pub async fn run_server(config: VizierConfig) -> Result<()> {
    let _ = std::fs::create_dir_all(PathBuf::from_str(&format!(
        "{}/db",
        config.workspace.clone()
    ))?);

    let deps = VizierDependencies::new(config.clone()).await?;

    init_workspace(config.workspace.clone());

    let mut agents = VizierAgents::new(
        config.workspace.clone(),
        config.agents.clone(),
        config.memory.clone(),
        config.tools.clone(),
        deps.clone(),
    )
    .await?;
    tokio::spawn(async move {
        if let Err(err) = agents.run().await {
            log::error!("{}", err);
        }
    });

    let channels = VizierChannels::new(config.channels.clone(), deps.clone())?;
    tokio::spawn(async move {
        if let Err(err) = channels.run().await {
            log::error!("{}", err);
        }
    });

    deps.run().await?;
    Ok(())
}

pub async fn run(args: RunArgs) -> Result<()> {
    let config = VizierConfig::load(args.config.clone())?;

    if args.tui {
        let handle = tokio::spawn(async move {
            run_server(config.clone()).await.unwrap();
        });

        tui::run(TuiArgs {
            base_url: None,
            config: Some(args.config.unwrap()),
        })
        .await?;

        handle.abort();
    } else {
        if env::var("RUST_LOG").is_err() {
            pretty_env_logger::formatted_builder()
                .filter_level(log::LevelFilter::Debug)
                .filter_module("rig", log::LevelFilter::Error)
                .filter_module("serenity", log::LevelFilter::Error)
                .filter_module("sqlx", log::LevelFilter::Error)
                .filter_module("reqwest", log::LevelFilter::Error)
                .filter_module("hyper", log::LevelFilter::Error)
                .filter_module("tungstenite", log::LevelFilter::Error)
                .filter_module("sqlx", log::LevelFilter::Error)
                .filter_module("h2", log::LevelFilter::Error)
                .filter_module("tracing", log::LevelFilter::Off)
                .filter_module("rustls", log::LevelFilter::Off)
                // .filter_module("surrealdb", log::LevelFilter::Off)
                .init();
        } else {
            pretty_env_logger::init();
        }

        run_server(config.clone()).await?;
    }

    Ok(())
}

pub fn init_workspace(path: String) {
    let boot_path = PathBuf::from(format!("{}/BOOT.md", path.clone()));
    let user_path = PathBuf::from(format!("{}/USER.md", path.clone()));
    let agent_path = PathBuf::from(format!("{}/AGENT.md", path.clone()));
    let ident_path = PathBuf::from(format!("{}/IDENT.md", path.clone()));

    let create_file_if_not_exists = |path: PathBuf, content: &str| {
        if !path.exists() {
            let _ = fs::write(path, content);
        }
    };

    let path = PathBuf::from(&path);

    if !path.exists() {
        let _ = std::fs::create_dir_all(path);
    }

    create_file_if_not_exists(boot_path, BOOT_MD);
    create_file_if_not_exists(user_path, USER_MD);
    create_file_if_not_exists(agent_path, AGENT_MD);
    create_file_if_not_exists(ident_path, IDENT_MD);
}
