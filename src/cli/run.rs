use anyhow::Result;
use clap::Args;

use crate::{
    agent::VizierAgents, channels::VizierChannels, config::VizierConfig,
    dependencies::VizierDependencies, scheduler::VizierScheduler,
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
}

pub async fn run_server(config: VizierConfig) -> Result<()> {
    let deps = VizierDependencies::new(config.clone()).await?;

    let mut scheduler = VizierScheduler::new(deps.clone()).await?;
    tokio::spawn(async move {
        if let Err(err) = scheduler.run().await {
            log::error!("{}", err);
        }
    });

    let mut agents = VizierAgents::new(deps.clone()).await?;
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

    log::info!("vizier is running!");
    deps.run().await?;
    Ok(())
}

pub async fn run(args: RunArgs) -> Result<()> {
    let config = VizierConfig::load(args.config.clone())?;

    run_server(config.clone()).await?;

    Ok(())
}
