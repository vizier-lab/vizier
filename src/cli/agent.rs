use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use duration_string::DurationString;
use inquire::{Confirm, CustomType, Select, Text};

use crate::{
    config::{
        VizierConfig,
        agent::{AgentConfig, AgentToolsConfig, MemoryConfig, ToolConfig},
        provider::ProviderVariant,
    },
    constant::AGENT_TEMPLATE,
};

#[derive(Debug, Parser, Clone)]
#[command(version, about = "Manage agents")]
pub struct AgentArgs {
    #[command(subcommand)]
    pub command: AgentSubcommand,
}

#[derive(Debug, Subcommand, Clone)]
pub enum AgentSubcommand {
    /// Create a new agent interactively
    New(AgentNewArgs),
}

#[derive(Debug, Args, Clone)]
pub struct AgentNewArgs {
    #[arg(
        short,
        long,
        value_name = "PATH",
        help = "path to workspace (defaults to config workspace)"
    )]
    pub path: Option<String>,
}

pub async fn agent(args: AgentArgs) -> Result<()> {
    match args.command {
        AgentSubcommand::New(args) => agent_new(args).await?,
    }
    Ok(())
}

pub async fn agent_new(args: AgentNewArgs) -> Result<()> {
    let config = VizierConfig::load(None::<PathBuf>)?;

    let workspace = match &args.path {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from(&config.workspace),
    };

    let agent_name = Text::new("Agent name:").with_default("MyAgent").prompt()?;

    let provider_names: Vec<String> = vec![
        "ollama".to_string(),
        "deepseek".to_string(),
        "openrouter".to_string(),
        "anthropic".to_string(),
        "openai".to_string(),
        "gemini".to_string(),
    ];

    let primary_provider_str = Select::new("Select provider:", provider_names.clone()).prompt()?;

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

    let model = Text::new("Model:").with_default(default_model).prompt()?;

    let agent_description = Text::new("Agent description (optional):")
        .with_default("Digital Assistant")
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

    let vector_memory_enabled = Confirm::new("Enable vector memory?")
        .with_default(true)
        .prompt()?;

    let brave_search_enabled = Confirm::new("Enable Brave search?")
        .with_default(false)
        .prompt()?;

    let discord_enabled = Confirm::new("Enable Discord?")
        .with_default(false)
        .prompt()?;

    let agent_file_name = format!(
        "{}.agent.md",
        agent_name
            .to_lowercase()
            .replace(' ', "_")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect::<String>()
    );

    let mut agent_path = workspace.clone().parent().unwrap().to_path_buf();
    agent_path.push(&agent_file_name);

    let agent = AgentConfig {
        name: agent_name.clone(),
        system_prompt: None,
        description: if agent_description.is_empty() {
            None
        } else {
            Some(agent_description)
        },
        provider: primary_provider.clone(),
        model: model.clone(),
        session_memory: MemoryConfig {
            max_capacity: memory_capacity,
        },
        thinking_depth,
        tools: AgentToolsConfig {
            timeout: DurationString::from_string("1m".into()).unwrap(),
            programmatic_sandbox: false,
            shell_access,
            brave_search: ToolConfig {
                enabled: brave_search_enabled,
            },
            vector_memory: ToolConfig {
                enabled: vector_memory_enabled,
            },
            discord: ToolConfig {
                enabled: discord_enabled,
            },
            telegram: ToolConfig { enabled: false },
            notify_primary_user: ToolConfig { enabled: true },
            mcp_servers: vec![],
        },
        silent_read_initiative_chance: 0.,
        show_thinking: Some(false),
        documents: vec![],
        include_documents: None,
        prompt_timeout: DurationString::from_string("5m".into()).unwrap(),
        heartbeat_interval: DurationString::from_string("30m".into()).unwrap(),
        dream_interval: DurationString::from_string("24h".into()).unwrap(),
        show_tool_calls: None,
    };

    println!("\n========== Agent Preview ==========\n");
    println!("Agent file: {}", agent_path.display());
    println!("Name: {}", agent.name);
    println!("Model: {} ({:?})", model, primary_provider);
    if agent.description.is_some() {
        println!("Description: {}", agent.description.as_ref().unwrap());
    }
    println!("Thinking depth: {}", thinking_depth);
    println!("Memory capacity: {}", memory_capacity);
    println!("\n--- Tools ---");
    println!("Shell access: {}", shell_access);
    println!("Vector memory: {}", vector_memory_enabled);
    println!("Brave search: {}", brave_search_enabled);
    println!("Discord: {}", discord_enabled);
    println!("\n==================================\n");

    let confirmed = Confirm::new("Create agent?").with_default(true).prompt()?;

    if !confirmed {
        println!("Agent creation cancelled.");
        return Ok(());
    }

    let agent_content = format!(
        r#"# {}

{}"#,
        agent.name, AGENT_TEMPLATE
    );

    let _ = crate::utils::markdown::write_markdown(&agent, agent_content, agent_path.clone())?;

    println!(
        "Successfully created agent at {}",
        agent_path.to_string_lossy()
    );

    Ok(())
}
