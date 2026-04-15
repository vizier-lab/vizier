use std::{collections::HashMap, fs, path::PathBuf};

use duration_string::DurationString;
use serde::{Deserialize, Serialize};

use crate::{config::provider::ProviderVariant, error::VizierError, utils};

pub type AgentConfigs = HashMap<String, AgentConfig>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentConfig {
    pub name: String,
    #[serde(skip)]
    pub system_prompt: Option<String>,
    pub description: Option<String>,
    pub provider: ProviderVariant,
    pub model: String,
    pub session_memory: MemoryConfig,
    pub thinking_depth: usize,
    pub tools: AgentToolsConfig,
    pub silent_read_initiative_chance: f32,
    pub show_thinking: Option<bool>,
    pub show_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_documents: Option<Vec<String>>,
    pub prompt_timeout: DurationString,
    #[serde(skip)]
    pub documents: Vec<String>,
    pub heartbeat_interval: DurationString,
}

impl AgentConfig {
    pub fn find_agent_configs(path: PathBuf) -> crate::Result<AgentConfigs> {
        let mut res = AgentConfigs::new();
        // find all .agent.md
        for entry in fs::read_dir(path).map_err(|err| VizierError(err.to_string()).into())? {
            let entry = entry.map_err(|err| VizierError(err.to_string()).into())?;

            let path = entry.path();
            if path.is_file() {
                if path.to_string_lossy().ends_with(".agent.md") {
                    let agent = Self::load_from_md(path.clone());

                    match agent {
                        Ok(agent) => {
                            let agent_id = path
                                .to_path_buf()
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap()
                                .replace(".agent.md", "");

                            res.insert(agent_id, agent);
                        }
                        Err(_) => {
                            log::warn!("failed to load {}", path.to_str().unwrap());
                        }
                    }
                }
            }
        }

        Ok(res)
    }

    fn load_from_md(s: PathBuf) -> crate::Result<Self> {
        // let raw_content = fs::read_to_string(&s)?;
        let (frontmatter, content) = utils::markdown::read_markdown::<AgentConfig>(s)
            .map_err(|err| VizierError(err.to_string()))?;

        let mut res: Self = frontmatter.clone();
        res.system_prompt = Some(content);

        // add all included documents
        let mut documents = vec![];
        if let Some(paths) = &res.include_documents {
            for path in paths {
                for entry in glob::glob(&path).map_err(|err| VizierError(err.to_string()))? {
                    let entry = entry.map_err(|err| VizierError(err.to_string()))?;
                    if !entry.is_file() {
                        continue;
                    }

                    documents.push(entry.to_string_lossy().to_string());
                }
            }
        }
        res.documents = documents;

        Ok(res)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryConfig {
    pub max_capacity: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AgentToolsConfig {
    pub timeout: DurationString,
    #[serde(default)]
    pub programmatic_sandbox: bool,
    #[serde(default)]
    pub shell_access: bool,
    #[serde(default)]
    pub brave_search: ToolConfig,
    #[serde(default)]
    pub vector_memory: ToolConfig,
    #[serde(default)]
    pub discord: ToolConfig,
    #[serde(default)]
    pub telegram: ToolConfig,
    #[serde(default)]
    pub notify_primary_user: ToolConfig,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolConfig {
    pub enabled: bool,
}
