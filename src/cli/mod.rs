use anyhow::Result;

use clap::{Parser, Subcommand};

mod agent;
mod init;
mod onboard;
mod run;

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run vizier agents, servers and channels
    Run(run::RunArgs),
    /// Onboard new user, and generate configurations
    Onboard(onboard::OnboardArgs),
    /// generate new config, non-interactively
    Configure,
    /// init a vizier directory
    Init,
    /// Manage agents
    Agent(agent::AgentArgs),
}

pub async fn start() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Onboard(args) => onboard::onboard(args.clone()).await?,
        Commands::Run(args) => run::run(args.clone()).await?,
        Commands::Init => init::init().await?,
        Commands::Agent(args) => agent::agent(args.clone()).await?,
        _ => {
            unimplemented!("TODO: unimplemented");
        }
    }

    Ok(())
}
