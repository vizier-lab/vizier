use std::{env, path::PathBuf};

use anyhow::Result;
use duration_string::DurationString;

use crate::config::{
    VizierConfig,
    agent::{AgentConfig, AgentToolsConfig, MemoryConfig, ToolConfig},
};

pub async fn init() -> Result<()> {
    let current_dir = env::current_dir()?;

    let mut config = VizierConfig::default();
    config.workspace = format!("{}/.vizier", current_dir.to_str().unwrap());

    let mut config_path = current_dir.clone();
    config_path.push(".vizier.yaml");
    config.save(config_path.clone(), "".into())?;

    let _ = VizierConfig::load(Some(config_path));

    init_default_agent(current_dir);

    Ok(())
}

pub fn init_default_agent(path: PathBuf) {
    let config = AgentConfig {
        name: "Vizier".to_string(),
        system_prompt: None,
        model: "qwen3.5:4b".into(),
        description: Some("Digital steward".into()),
        provider: crate::config::provider::ProviderVariant::ollama,
        prompt_timeout: DurationString::from_string("5m".into()).unwrap(),
        session_memory: MemoryConfig { max_capacity: 10 },
        thinking_depth: 10,
        tools: AgentToolsConfig {
            timeout: DurationString::from_string("1m".into()).unwrap(),
            python_interpreter: false,
            shell_access: false,
            brave_search: ToolConfig {
                enabled: false,
                programmatic_tool_call: false,
            },
            vector_memory: ToolConfig {
                enabled: true,
                programmatic_tool_call: false,
            },
            discord: ToolConfig {
                enabled: false,
                programmatic_tool_call: false,
            },
            mcp_servers: vec![],
        },
        silent_read_initiative_chance: 0.,
        show_thinking: Some(false),
        documents: vec![],
        include_documents: None,
    };

    let content = format!(
        r#"# {}
You are a 21st digital steward, your duty is to answer any questions from the user.
Utilize any documents and memories at your disposal"#,
        config.name
    );

    let mut target_path = path.clone();
    target_path.push("vizier.agent.md");
    let _ = crate::utils::markdown::write_markdown(&config, content, target_path);
}
