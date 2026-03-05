use std::{path::PathBuf, str::FromStr};

use anyhow::Result;
use clap::Args;
use dirs::home_dir;
use duration_string::DurationString;
use inquire::{Confirm, CustomType, Password, Select, Text};

use crate::config::{
    AgentConfig, AgentConfigs, ChannelsConfig, DiscordChannelConfig, HTTPChannelConfig,
    MemoryConfig, ModelConfig, ToolsConfig, VizierConfig,
};

#[derive(Debug, Args, Clone)]
pub struct OnboardArgs {
    #[arg(short, long, value_name = "PATH", help = "path to workspace")]
    pub path: Option<String>,
}

pub async fn onboard(args: OnboardArgs) -> Result<()> {
    let home = home_dir().unwrap();
    let default_workspace = format!("{}/.vizier", home.to_str().unwrap());

    let workspace = match args.path {
        None => Text::new("agent workspace directory: ")
            .with_default(&default_workspace)
            .prompt()?,
        Some(path) => path,
    };

    // agents config
    // setup primary agetn
    let name = Text::new("give your agent a name: ")
        .with_default("vizier")
        .prompt()?;

    let provider_opts = vec!["ollama", "openrouter", "deepseek"];
    let provider = Select::new("your llm provider: ", provider_opts).prompt()?;

    let mut llm_host = "".to_string();
    let mut llm_api_key = "".to_string();

    if provider == "ollama" {
        let host = Text::new("Ollama base url: ")
            .with_default("http://localhost:11434")
            .prompt()?;

        llm_host = host;
    } else {
        let api_key = Password::new(&format!("{} api key: ", provider))
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .without_confirmation()
            .prompt()?;
        llm_api_key = api_key;
    }
    let model_name = Text::new("llm model: ")
        .with_default("anthropic/claude-haiku-4")
        .prompt()?;

    let primary = AgentConfig {
        name,
        model: ModelConfig {
            provider: provider.into(),
            name: model_name,
            base_url: llm_host.into(),
            api_key: llm_api_key.into(),
        },
        session_ttl: DurationString::from_str("30m")?.into(),
    };

    let agents = AgentConfigs { primary };

    let enable_discord = Confirm::new("Do you want to enable discord chatbot: ")
        .with_default(true)
        .prompt()?;

    let discord = if enable_discord {
        let token = Password::new("dicord api token: ")
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .without_confirmation()
            .prompt()?;

        Some(DiscordChannelConfig { token })
    } else {
        None
    };

    let enable_websocket = Confirm::new("Do you want to enable websocket: ")
        .with_default(true)
        .prompt()?;

    let http = if enable_websocket {
        let port = CustomType::<u32>::new("websocket port: ")
            .with_formatter(&|i| format!("{i}"))
            .prompt()?;

        Some(HTTPChannelConfig { port })
    } else {
        None
    };

    let channels = ChannelsConfig { discord, http };

    let memory = MemoryConfig {
        session_memory_recall_depth: 10,
    };

    let tools = ToolsConfig {
        vector_memory: None,
        brave_search: None,
        turn_depth: 100,
        dangerously_enable_cli_access: false,
    };

    let vizier = VizierConfig {
        workspace: workspace.clone(),
        agents,
        channels,
        tools,
        memory,
    };

    let mut path = PathBuf::from_str(&workspace.clone())?;
    path.push("config.toml");
    vizier.save(path, COMMENTED_CONFIGS.to_string())?;

    println!(
        r#"
🎉 Great news! Your configuration has been created at: {}/config.toml

📚 Before getting started, we recommend reviewing these documents:
🧠 {}/BOOT.md - Your operational memory and doctrine
🤖 {}/AGENT.md - Core operating framework and self-update system
👑 {}/USER.md - Primary user profile and information
👤 {}/IDENT.md - Agent identity and personality definition
        "#,
        workspace, workspace, workspace, workspace, workspace,
    );
    println!(
        "to run: `{}`",
        if workspace == default_workspace {
            "vizier run".to_string()
        } else {
            format!("vizier run --config {}/config.toml", workspace)
        }
    );

    Ok(())
}

const COMMENTED_CONFIGS: &'static str = r#"
# allow the agent to search the web
# [vizier.tools.brave_search]
# api_key = "YOUR_BRAVE_API_KEY"
#
# enabling dynamic vector memory 
# [vizier.tools.vector_memory]
# model = { provider = "ollama", name = "nomic-embed-text", base_url = "http://localhost:11434" }
"#;
