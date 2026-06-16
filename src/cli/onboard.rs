use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use clap::Args;
use inquire::{Confirm, Password, Select, Text};
use termimad::print_text;

use crate::config::{
    ChannelsConfig, HTTPChannelConfig, VizierConfig,
    provider::ProviderConfig,
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

    let mut providers = ProviderConfig::default();

    print_text("\n> **Note:** You can add more providers later in the WebUI settings.\n");

    let provider_type = Select::new(
        "Select primary provider:",
        vec![
            "mistralrs",
            "ollama",
            "deepseek",
            "openrouter",
            "anthropic",
            "openai",
            "gemini",
            "mimo",
            "llama_cpp",
        ],
    )
    .prompt()?;

    match provider_type {
        "mistralrs" => {
            providers.mistralrs = Some(crate::config::provider::MistralrsProviderConfig {
                enabled: true,
            });
        }
        "ollama" => {
            let base_url = Text::new("Ollama base URL:")
                .with_default("http://localhost:11434")
                .prompt()?;
            providers.ollama = Some(crate::config::provider::OllamaProviderConfig { base_url });
        }
        "deepseek" => {
            let api_key = Password::new("Deepseek API key:").prompt()?;
            providers.deepseek = Some(crate::config::provider::DeepseekProviderConfig { api_key });
        }
        "openrouter" => {
            let api_key = Password::new("OpenRouter API key:").prompt()?;
            providers.openrouter =
                Some(crate::config::provider::OpenRouterProviderConfig { api_key });
        }
        "anthropic" => {
            let api_key = Password::new("Anthropic API key:").prompt()?;
            providers.anthropic = Some(crate::config::provider::AnthropicProviderConfig { api_key });
        }
        "openai" => {
            let api_key = Password::new("OpenAI API key:").prompt()?;
            providers.openai = Some(crate::config::provider::OpenAIProviderConfig { api_key });
        }
        "gemini" => {
            let api_key = Password::new("Gemini API key:").prompt()?;
            providers.gemini = Some(crate::config::provider::GeminiProviderConfig { api_key });
        }
        "mimo" => {
            let api_key = Password::new("Xiaomi MiMo API key:").prompt()?;
            providers.mimo = Some(crate::config::provider::MimoProviderConfig { api_key });
        }
        "llama_cpp" => {
            let base_url = Text::new("Llama.cpp base URL:")
                .with_default("http://localhost:8080")
                .prompt()?;
            providers.llama_cpp =
                Some(crate::config::provider::LlamaCppProviderConfig { base_url });
        }
        _ => unreachable!(),
    }

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
        | **Provider** | `{}` |\n\
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
        providers,
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
        | **WebUI** | `http://localhost:{}` |\n\n\
        Open the WebUI and follow the onboarding flow to create your first user account.\n\
        You can configure embedding and indexer per-agent in agent settings.\n",
        config_path.display(),
        port,
    ));

    Ok(())
}
