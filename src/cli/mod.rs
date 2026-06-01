use anyhow::Result;

use clap::{Parser, Subcommand};

mod onboard;
mod run;
mod shutdown;
mod skill;

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
    /// Manage skills (install, list, uninstall, update)
    Skill(skill::SkillArgs),
}

pub fn start() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Onboard(args) => onboard::onboard(args.clone())?,
        Commands::Run(args) => run::run(args.clone())?,
        Commands::Shutdown(args) => shutdown::shutdown(args.clone())?,
        Commands::Skill(args) => skill::skill(args.clone())?,
    }

    Ok(())
}
