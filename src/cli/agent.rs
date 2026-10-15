use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Args, Subcommand};

use crate::{command::VizierCommandClient, config::VizierConfig, schema::{AgentRunState, ChannelState, CommandRequest}};

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

                let y = "\x1b[33m";
                for status in &statuses {
                    let (indicator, color) = match status.state {
                        AgentRunState::Running => ("\u{25cf} online", g),
                        AgentRunState::Starting => ("\u{25cc} starting", y),
                        AgentRunState::Failed => ("\u{25cb} failed", r),
                        AgentRunState::Stopped => ("\u{25cb} stopped", r),
                    };
                    println!("  {:<20} {}{}{}", status.agent_id, color, indicator, reset);
                    if let Some(model) = &status.model {
                        let context = model
                            .context_window
                            .map(|w| format!("context {w} ({:?})", model.context_source).to_lowercase())
                            .unwrap_or_else(|| "context unknown".into());
                        println!(
                            "  {:<20} {}{}/{} · {}{}",
                            "", d, model.provider, model.model, context, reset
                        );
                    }
                    for warning in &status.warnings {
                        println!("  {:<20} {}! {}{}", "", y, warning, reset);
                    }
                    if let Some(error) = &status.last_error {
                        let retry = status
                            .next_retry_at
                            .map(|at| format!(", retrying at {}", at.format("%H:%M:%S UTC")))
                            .unwrap_or_default();
                        println!(
                            "  {:<20} {}{} (failure {}{}){}",
                            "", d, error, status.failures, retry, reset
                        );
                    }
                    for channel in &status.channels {
                        let (indicator, color) = match channel.state {
                            ChannelState::Running => ("\u{25cf} running", g),
                            ChannelState::Reconnecting => ("\u{25cc} reconnecting", y),
                            ChannelState::Failed => ("\u{25cb} failed", r),
                        };
                        let error = channel
                            .last_error
                            .as_ref()
                            .map(|e| format!(" {d}({e}){reset}"))
                            .unwrap_or_default();
                        println!(
                            "  {:<20}   {} {}{}{}{}",
                            "", channel.platform, color, indicator, reset, error
                        );
                    }
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
