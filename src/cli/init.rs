use std::{env, path::PathBuf};

use anyhow::Result;
use duration_string::DurationString;

use crate::{
    config::{
        VizierConfig,
        agent::{AgentConfig, AgentToolsConfig, MemoryConfig, ToolConfig},
    },
    constant::AGENT_TEMPLATE,
    utils::build_path,
};

pub async fn init() -> Result<()> {
    let current_dir = env::current_dir()?;

    let mut config = VizierConfig::default();
    config.workspace = build_path(current_dir.to_str().unwrap(), &[".vizier"])
        .to_string_lossy()
        .to_string();

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
            programmatic_sandbox: false,
            timeout: DurationString::from_string("1m".into()).unwrap(),
            shell_access: false,
            brave_search: ToolConfig { enabled: false },
            vector_memory: ToolConfig { enabled: true },
            discord: ToolConfig { enabled: false },
            telegram: ToolConfig { enabled: false },
            notify_primary_user: ToolConfig { enabled: true },
            mcp_servers: vec![],
        },
        silent_read_initiative_chance: 0.,
        show_thinking: Some(false),
        documents: vec![],
        include_documents: None,
        heartbeat_interval: DurationString::from_string("30m".into()).unwrap(),
        show_tool_calls: None,
    };

    let content = format!(
        r#"# {}

{}"#,
        config.name, AGENT_TEMPLATE
    );

    let mut target_path = path.clone();
    target_path.push("vizier.agent.md");
    let _ = crate::utils::markdown::write_markdown(&config, content, target_path);
}
