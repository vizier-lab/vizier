use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use clap::Args;
use inquire::{Confirm, Password, Select, Text};
use termimad::print_text;

use crate::config::{
    ChannelsConfig, HTTPChannelConfig, VizierConfig, provider::ProviderId,
    storage::StorageConfig,
};

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

    print_text("\n> **Note:** Provider keys are not written to the config file. Add them in Settings › Providers after the first start, or set the environment variable shown below.\n");

    let provider_type = Select::new(
        "Select primary provider:",
        vec![
            "ollama",
            "deepseek",
            "openrouter",
            "anthropic",
            "openai",
            "google",
            "xiaomi",
            "llama-cpp",
        ],
    )
    .prompt()?;

    let provider_hint = provider_env_hint(provider_type);

    let storage_type = Select::new("Storage type:", vec!["Filesystem", "SQLite"]).prompt()?;

    let storage = if storage_type == "SQLite" {
        StorageConfig::Sqlite
    } else {
        StorageConfig::Filesystem
    };

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
        | **Provider** | `{}` ({}) |\n\
        | **Storage** | `{}` |\n\
        | **Worker threads** | `{}` |\n\
        | **WS idle timeout** | `{}s` |\n\
        \n\
        > **Note:** Embedding and indexer are configured **per agent** in the WebUI.\n\
        ---\n",
        workspace_path.display(),
        config_path.display(),
        port,
        provider_type,
        provider_hint,
        storage_type,
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
        storage,
        channels: ChannelsConfig {
            http: Some(HTTPChannelConfig {
                port,
                jwt_secret,
                jwt_expiry_hours: 720,
                ws_idle_timeout_secs,
            }),
        },
        worker_threads,
    };

    config.save(config_path.clone(), "".into())?;

    print_text(&format!(
        "\n### Onboarding complete!\n\n\
        Configuration saved to `{}`\n\n\
        | | |\n\
        |---|---|\n\
        | **Start server** | `vizier run` |\n\
        | **WebUI** | `http://localhost:{}` |\n\
        | **Provider** | {} |\n\n\
        Open the WebUI and follow the onboarding flow to create your first user account.\n\
        You can configure embedding and indexer per-agent in agent settings.\n",
        config_path.display(),
        port,
        provider_hint,
    ));

    Ok(())
}

/// How to give the chosen provider its credentials: the environment variable from the
/// catalogue (or the built-in's), or Settings › Providers.
fn provider_env_hint(provider: &str) -> String {
    let Ok(id) = ProviderId::parse(provider) else {
        return "add it in Settings › Providers".into();
    };
    let env = crate::catalogue::builtin::builtin(&id)
        .and_then(|b| b.env.first().map(|e| e.to_string()))
        .or_else(|| {
            crate::catalogue::seed::load_seed()
                .providers
                .get(&id)
                .and_then(|p| p.env.iter().find(|e| e.contains("KEY") || e.contains("TOKEN")).cloned())
        });
    match env {
        Some(var) => format!("set `{var}`, or add it in Settings › Providers"),
        None => "add it in Settings › Providers".into(),
    }
}
