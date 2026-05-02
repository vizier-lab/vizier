use anyhow::Result;

use clap::{Parser, Subcommand};

mod agent;
mod init;
mod onboard;
mod run;
mod shutdown;

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
    /// Stops vizier agents, servers and channels
    Shutdown(shutdown::ShutdownArgs),
    /// Onboard new user, and generate configurations
    Onboard(onboard::OnboardArgs),
    /// generate new config, non-interactively
    Configure,
    /// init a vizier directory
    Init,
    /// Manage agents
    Agent(agent::AgentArgs),
}

pub fn start() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Onboard(args) => onboard::onboard(args.clone())?,
        Commands::Run(args) => run::run(args.clone())?,
        Commands::Shutdown(args) => shutdown::shutdown(args.clone())?,
        Commands::Init => init::init()?,
        Commands::Agent(args) => agent::agent(args.clone())?,
        _ => {
            unimplemented!("TODO: unimplemented");
        }
    }

    Ok(())
}
