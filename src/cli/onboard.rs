use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use clap::Args;
use duration_string::DurationString;
use inquire::{Confirm, CustomType, Password, Select, Text};

use crate::{
    config::{
        ChannelsConfig, DiscordChannelConfig, HTTPChannelConfig, VizierConfig,
        agent::{AgentConfig, AgentToolsConfig, MemoryConfig, ToolConfig},
        provider::{ProviderConfig, ProviderVariant},
        storage::{DocumentIndexerConfig, StorageConfig},
        tools::{BraveSearchConfig, ToolsConfig},
        user::UserConfig,
    },
    constant::AGENT_TEMPLATE,
};

#[derive(Debug, Args, Clone)]
pub struct OnboardArgs {
    #[arg(short, long, value_name = "PATH", help = "path to workspace")]
    pub path: Option<String>,
}

pub async fn onboard(args: OnboardArgs) -> Result<()> {
    let workspace = match &args.path {
        Some(p) => p.clone(),
        None => Text::new("Workspace path:")
            .with_default(&std::env::current_dir()?.to_string_lossy())
            .prompt()?,
    };

    let workspace_path = if workspace.starts_with("~") {
        dirs::home_dir()
            .unwrap()
            .join(workspace.strip_prefix("~").unwrap())
    } else {
        PathBuf::from(&workspace)
    };

    let user_name = Text::new("Your name:").with_default("admin").prompt()?;

    let mut providers = ProviderConfig::default();
    let mut provider_names: Vec<String> = Vec::new();
    let mut add_more = true;

    while add_more {
        let provider_type = Select::new(
            "Select provider type:",
            vec![
                "ollama".to_string(),
                "deepseek".to_string(),
                "openrouter".to_string(),
                "anthropic".to_string(),
                "openai".to_string(),
                "gemini".to_string(),
            ],
        )
        .prompt()?;

        let variant = match provider_type.as_str() {
            "ollama" => {
                let base_url = Text::new("Ollama base URL:")
                    .with_default("http://localhost:11434")
                    .prompt()?;
                providers.ollama = Some(crate::config::provider::OllamaProviderConfig { base_url });
                ProviderVariant::ollama
            }
            "deepseek" => {
                let api_key = Password::new("Deepseek API Key:").prompt()?;
                providers.deepseek =
                    Some(crate::config::provider::DeepseekProviderConfig { api_key });
                ProviderVariant::deepseek
            }
            "openrouter" => {
                let api_key = Password::new("OpenRouter API Key:").prompt()?;
                providers.openrouter =
                    Some(crate::config::provider::OpenRouterProviderConfig { api_key });
                ProviderVariant::openrouter
            }
            "anthropic" => {
                let api_key = Password::new("Anthropic API Key:").prompt()?;
                providers.anthropic =
                    Some(crate::config::provider::AnthropicProviderConfig { api_key });
                ProviderVariant::anthropic
            }
            "openai" => {
                let api_key = Password::new("OpenAI API Key:").prompt()?;
                let base_url = Text::new("OpenAI base URL (optional):")
                    .with_default("")
                    .prompt()?;
                providers.openai = Some(crate::config::provider::OpenAIProviderConfig {
                    api_key,
                    base_url: if base_url.is_empty() {
                        None
                    } else {
                        Some(base_url)
                    },
                });
                ProviderVariant::openai
            }
            "gemini" => {
                let api_key = Password::new("Gemini API Key:").prompt()?;
                providers.gemini = Some(crate::config::provider::GeminiProviderConfig { api_key });
                ProviderVariant::gemini
            }
            _ => unreachable!(),
        };

        provider_names.push(format!("{} ({:?})", provider_type, variant));

        add_more = Confirm::new("Add another provider?")
            .with_default(false)
            .prompt()?;
    }

    if provider_names.is_empty() {
        providers.ollama = Some(crate::config::provider::OllamaProviderConfig::default());
        provider_names.push("ollama (ollama)".to_string());
    }

    let primary_provider_str =
        Select::new("Select primary provider:", provider_names.clone()).prompt()?;

    let primary_provider = if primary_provider_str.contains("ollama") {
        ProviderVariant::ollama
    } else if primary_provider_str.contains("deepseek") {
        ProviderVariant::deepseek
    } else if primary_provider_str.contains("openrouter") {
        ProviderVariant::openrouter
    } else if primary_provider_str.contains("anthropic") {
        ProviderVariant::anthropic
    } else if primary_provider_str.contains("openai") {
        ProviderVariant::openai
    } else {
        ProviderVariant::gemini
    };

    let default_model = match primary_provider {
        ProviderVariant::ollama => "qwen3.5:4b",
        ProviderVariant::deepseek => "deepseek-chat",
        ProviderVariant::openrouter => "anthropic/claude-3-haiku",
        ProviderVariant::anthropic => "claude-3-haiku-20240307",
        ProviderVariant::openai => "gpt-4o-mini",
        ProviderVariant::gemini => "gemini-2.0-flash",
    };

    let agent_name = Text::new("Agent name:").with_default("Vizier").prompt()?;
    let model = Text::new("Model:").with_default(default_model).prompt()?;
    let agent_description = Text::new("Agent description (optional):")
        .with_default("Digital Steward")
        .prompt()?;
    let thinking_depth: usize = CustomType::new("Thinking depth:")
        .with_default(10)
        .prompt()?;
    let memory_capacity: usize = CustomType::new("Session memory capacity:")
        .with_default(10)
        .prompt()?;

    let shell_access = Confirm::new("Enable shell access?")
        .with_default(false)
        .prompt()?;

    let brave_search_enabled = Confirm::new("Enable Brave search?")
        .with_default(false)
        .prompt()?;

    let brave_search_api_key = if brave_search_enabled {
        Some(Password::new("Brave API Key:").prompt()?)
    } else {
        None
    };

    let vector_memory_enabled = Confirm::new("Enable vector memory?")
        .with_default(true)
        .prompt()?;

    let discord_enabled = Confirm::new("Enable Discord?")
        .with_default(false)
        .prompt()?;

    let discord_token = if discord_enabled {
        Some(Password::new("Discord bot token:").prompt()?)
    } else {
        None
    };

    let storage_type = Select::new(
        "Storage type:",
        vec!["Filesystem".to_string(), "InMem".to_string()],
    )
    .prompt()?;

    let storage = if storage_type == "Filesystem" {
        StorageConfig::Filesystem(DocumentIndexerConfig::InMem)
    } else {
        StorageConfig::Filesystem(DocumentIndexerConfig::InMem)
    };

    let agent_file_name = format!(
        "{}.agent.md",
        agent_name
            .to_lowercase()
            .replace(' ', "_")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect::<String>()
    );

    let mut config_path = workspace_path.clone();
    config_path.push(".vizier.yaml");

    let mut agent_path = workspace_path.clone();
    agent_path.push(&agent_file_name);

    println!("\n========== Configuration Preview ==========\n");
    println!("Workspace: {}", workspace_path.display());
    println!("Config file: {}", config_path.display());
    println!("Agent file: {}\n", agent_path.display());
    println!("--- User ---");
    println!("Name: {}", user_name);
    println!("\n--- Providers ---");
    if providers.ollama.is_some() {
        println!(
            "  - ollama: {}",
            providers.ollama.as_ref().unwrap().base_url
        );
    }
    if providers.deepseek.is_some() {
        println!("  - deepseek: [API KEY SET]");
    }
    if providers.openrouter.is_some() {
        println!("  - openrouter: [API KEY SET]");
    }
    if providers.anthropic.is_some() {
        println!("  - anthropic: [API KEY SET]");
    }
    if providers.openai.is_some() {
        println!("  - openai: [API KEY SET]");
    }
    if providers.gemini.is_some() {
        println!("  - gemini: [API KEY SET]");
    }
    println!("\n--- Primary Provider ---");
    println!("  {:?}\n", primary_provider);
    println!("--- Agent: {} ---", agent_name);
    println!("Model: {}", model);
    if !agent_description.is_empty() {
        println!("Description: {}", agent_description);
    }
    println!("Thinking depth: {}", thinking_depth);
    println!("Memory capacity: {}", memory_capacity);
    println!("\n--- Tools ---");
    println!("Shell access: {}", shell_access);
    println!("Brave search: {}", brave_search_enabled);
    println!("Vector memory: {}", vector_memory_enabled);
    println!("Discord: {}", discord_enabled);
    println!("\n--- Storage ---");
    println!("{}", storage_type);
    println!("\n==========================================\n");

    let confirmed = Confirm::new("Save configuration?")
        .with_default(true)
        .prompt()?;

    if !confirmed {
        println!("Onboarding cancelled.");
        return Ok(());
    }

    let config = VizierConfig {
        workspace: workspace.clone(),
        embedding: Some(crate::config::embedding::EmbeddingConfig::Local {
            model: crate::config::embedding::LocalEmbeddingModelVariant::AllMiniLml6V2,
        }),
        primary_user: UserConfig {
            name: user_name,
            discord_id: "".into(),
            discord_username: "".into(),
            alias: vec![],
        },
        providers,
        storage,
        agents: HashMap::new(),
        channels: ChannelsConfig {
            discord: discord_token.map(|token| {
                HashMap::from([("vizier".to_string(), DiscordChannelConfig { token })])
            }),
            http: Some(HTTPChannelConfig {
                port: 9999,
                jwt_secret: "${VIZIER_JWT_SECRET}".into(),
                jwt_expiry_hours: 720,
            }),
        },
        tools: ToolsConfig {
            brave_search: if brave_search_enabled {
                Some(BraveSearchConfig {
                    api_key: brave_search_api_key.unwrap(),
                    safesearch: true,
                })
            } else {
                None
            },
            mcp_servers: HashMap::new(),
        },
        shell: crate::config::shell::ShellConfig::Local(crate::config::shell::LocalShellConfig {
            path: ".".into(),
            env: None,
        }),
    };

    let agent = AgentConfig {
        name: agent_name.clone(),
        system_prompt: None,
        description: if agent_description.is_empty() {
            None
        } else {
            Some(agent_description)
        },
        provider: primary_provider,
        model: model.clone(),
        session_memory: MemoryConfig {
            max_capacity: memory_capacity,
        },
        thinking_depth,
        tools: AgentToolsConfig {
            timeout: DurationString::from_string("1m".into()).unwrap(),
            python_interpreter: false,
            shell_access,
            brave_search: ToolConfig {
                enabled: brave_search_enabled,
                programmatic_tool_call: false,
            },
            vector_memory: ToolConfig {
                enabled: vector_memory_enabled,
                programmatic_tool_call: false,
            },
            discord: ToolConfig {
                enabled: discord_enabled,
                programmatic_tool_call: false,
            },
            mcp_servers: vec![],
        },
        silent_read_initiative_chance: 0.,
        show_thinking: Some(false),
        documents: vec![],
        include_documents: None,
        prompt_timeout: DurationString::from_string("5m".into()).unwrap(),
        heartbeat_interval: DurationString::from_string("30m".into()).unwrap(),
    };

    config.save(config_path.clone(), "".into())?;

    let agent_content = format!(
        r#"# {}

{}"#,
        agent_name, AGENT_TEMPLATE
    );

    let _ = crate::utils::markdown::write_markdown(&agent, agent_content, agent_path);

    println!(
        "Successfully onboarded! Configuration saved to {}",
        config_path.display()
    );

    Ok(())
}
