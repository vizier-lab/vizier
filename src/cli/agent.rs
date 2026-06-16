use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Args, Subcommand};

use crate::{command::VizierCommandClient, config::VizierConfig, schema::CommandRequest};

#[derive(Debug, Args, Clone)]
pub struct AgentArgs {
    #[arg(
        short,
        long,
        value_name = "PATH",
        value_hint = clap::ValueHint::DirPath,
        help = "path to .vizier.yaml or .vizier/config.yaml (optional, uses defaults if omitted)",
    )]
    config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    pub command: AgentCommand,
}

#[derive(Debug, Subcommand, Clone)]
pub enum AgentCommand {
    /// Show running agent processes and their status
    Ps,
}

pub fn agent(args: AgentArgs) -> Result<()> {
    match args.command {
        AgentCommand::Ps => ps(args.config),
    }
}

fn ps(config_path: Option<std::path::PathBuf>) -> Result<()> {
    let config = VizierConfig::load(config_path)?;

    let sock_path = PathBuf::from(&config.workspace)
        .join(".runtime")
        .join(".vizier.sock");
    if !sock_path.exists() {
        bail!("no vizier daemon running at {}", sock_path.display());
    }

    tokio::runtime::Runtime::new()?.block_on(async move {
        let client = VizierCommandClient::new(config)?;

        match client.send_command(CommandRequest::HealthCheck).await {
            Ok(crate::schema::CommandResponse::Ok(body)) => {
                let statuses: Vec<crate::schema::AgentHealthStatus> =
                    serde_json::from_str(&body).unwrap_or_default();

                if statuses.is_empty() {
                    println!("No agents running.");
                    return Ok(());
                }

                let d = "\x1b[2m";
                let g = "\x1b[32m";
                let r = "\x1b[31m";
                let reset = "\x1b[0m";

                println!();
                println!("  {}Agent ID           Status{}", d, reset);
                println!("  {}------------------- --------{}", d, reset);

                for status in &statuses {
                    let (indicator, color) = if status.alive {
                        ("\u{25cf} online", g)
                    } else {
                        ("\u{25cb} offline", r)
                    };
                    println!("  {:<20} {}{}{}", status.agent_id, color, indicator, reset);
                }

                let online = statuses.iter().filter(|s| s.alive).count();
                let total = statuses.len();
                println!();
                println!(
                    "  {}{} agent(s): {} online, {} offline{}",
                    d,
                    total,
                    online,
                    total - online,
                    reset
                );
                println!();

                Ok(())
            }
            Ok(crate::schema::CommandResponse::Error(e)) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("Failed to connect to vizier: {}", e);
                std::process::exit(1);
            }
        }
    })
}
