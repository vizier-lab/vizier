use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use inquire::{Confirm, Text};
use termimad::print_text;

use crate::config::{ChannelsConfig, HTTPChannelConfig, VizierConfig, storage::StorageConfig};

#[derive(Debug, Args, Clone)]
pub struct OnboardArgs {
    #[arg(short, long, value_name = "PATH", help = "path to workspace")]
    pub path: Option<String>,
}

pub fn onboard(args: OnboardArgs) -> Result<()> {
    crate::utils::logo::print_logo();

    let d = "\x1b[2m";
    let r = "\x1b[0m";

    println!("  Welcome! Let's set up your agent workspace.");
    println!("  This will generate a {d}.vizier.yaml{r} config.");
    println!("  You can create your user account later via the WebUI.");
    println!();
    let workspace = match &args.path {
        Some(p) => p.clone(),
        None => Text::new("Workspace path:")
            .with_default(&std::env::current_dir()?.to_string_lossy())
            .prompt()?,
    };

    let workspace_path = if workspace.starts_with('~') {
        dirs::home_dir()
            .unwrap()
            .join(workspace.strip_prefix('~').unwrap())
    } else {
        PathBuf::from(&workspace)
    };

    let port: u32 = Text::new("HTTP port:")
        .with_default("9999")
        .prompt()?
        .parse()
        .unwrap_or(9999);

    let default_jwt = nanoid::nanoid!(32);
    let jwt_secret = Text::new("JWT secret:")
        .with_default(&default_jwt)
        .prompt()?;

    let worker_threads: usize = Text::new("Worker threads:")
        .with_default("4")
        .prompt()?
        .parse()
        .unwrap_or(4);

    let ws_idle_timeout_secs: u64 = Text::new("WebSocket idle timeout (seconds):")
        .with_default("300")
        .prompt()?
        .parse()
        .unwrap_or(300);

    let mut config_path = workspace_path.clone();
    config_path.push(".vizier.yaml");

    print_text(&format!(
        "\n---\n### Configuration Preview\n\n\
        | | |\n\
        |---|---|\n\
        | **Workspace** | `{}` |\n\
        | **Config file** | `{}` |\n\
        | **HTTP port** | `{}` |\n\
        | **Worker threads** | `{}` |\n\
        | **WS idle timeout** | `{}s` |\n\
        \n\
        > **Note:** Storage is SQLite. Providers are connected in the WebUI (Settings › Providers),\n\
        > and embedding and indexer are configured **per agent**.\n\
        ---\n",
        workspace_path.display(),
        config_path.display(),
        port,
        worker_threads,
        ws_idle_timeout_secs,
    ));

    let confirmed = Confirm::new("Save configuration?")
        .with_default(true)
        .prompt()?;

    if !confirmed {
        println!("Onboarding cancelled.");
        return Ok(());
    }

    let config = VizierConfig {
        workspace: workspace.clone(),
        providers: None,
        storage: StorageConfig::Sqlite,
        channels: ChannelsConfig {
            http: Some(HTTPChannelConfig {
                port,
                jwt_secret,
                jwt_expiry_hours: 720,
                ws_idle_timeout_secs,
            }),
        },
        worker_threads,
        catalogue: Default::default(),
    };

    config.save(config_path.clone(), "".into())?;

    print_text(&format!(
        "\n### Onboarding complete!\n\n\
        Configuration saved to `{}`\n\n\
        | | |\n\
        |---|---|\n\
        | **Start server** | `vizier run` |\n\
        | **WebUI** | `http://localhost:{}` |\n\n\
        Open the WebUI to create your admin account; the Home page then walks you through\n\
        connecting a provider and creating your first agent.\n",
        config_path.display(),
        port,
    ));

    Ok(())
}
