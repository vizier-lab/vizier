use std::{fs, str::FromStr};

use anyhow::Result;
use config::Config;
use duration_string::DurationString;
use serde::{Deserialize, Serialize};

pub mod embedding;

use crate::{config::embedding::VizierEmbeddingModel, constant};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryConfig {
    pub session_memory_recall_depth: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentConfig {
    pub name: String,
    pub model: ModelConfig,
    pub session_ttl: DurationString,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelConfig {
    pub provider: String,
    pub name: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub base_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChannelsConfig {
    pub discord: Option<DiscordChannelConfig>,
    pub http: Option<HTTPChannelConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiscordChannelConfig {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HTTPChannelConfig {
    pub port: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentConfigs {
    pub primary: AgentConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolsConfig {
    pub dangerously_enable_cli_access: bool,
    pub brave_search: Option<BraveSearchConfig>,
    pub vector_memory: Option<VectorMemoryConfig>,
    #[serde(default)]
    pub turn_depth: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BraveSearchConfig {
    pub api_key: String,
    #[serde(default)]
    pub safesearch: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VectorMemoryConfig {
    pub model: VizierEmbeddingModel,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VizierConfig {
    pub workspace: String,
    pub agents: AgentConfigs,
    pub channels: ChannelsConfig,
    pub memory: MemoryConfig,
    pub tools: ToolsConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AllConfig {
    vizier: VizierConfig,
}

impl VizierConfig {
    pub fn load(path: Option<std::path::PathBuf>) -> Result<Self> {
        let mut default_path = dirs::home_dir().unwrap();
        default_path.push(std::path::PathBuf::from_str(constant::DEFAULT_CONFIG_PATH).unwrap());

        let path = path.unwrap_or_else(|| {
            log::warn!(
                "config path not inputed, fallback to {:?}",
                default_path.to_str().unwrap()
            );

            default_path
        });

        if !path.exists() {
            log::warn!(
                "{} not found, generating a new config file",
                path.to_str().unwrap()
            );

            Self::create_file(path.clone())?;
        }

        let settings = Config::builder()
            .add_source(config::File::from(path.clone()))
            .build()?;

        log::info!("config loaded: {:?}", path.to_str().unwrap());
        let config = settings.try_deserialize::<AllConfig>()?;

        Ok(config.vizier)
    }

    pub fn create_file(path: std::path::PathBuf) -> Result<()> {
        if let Some(parent_dir) = path.parent() {
            let _ = std::fs::create_dir_all(parent_dir)?;
        }

        let _ = fs::write(&path, constant::DEFAULT_CONFIG_TOML)?;

        Ok(())
    }

    pub fn save(&self, path: std::path::PathBuf, addition: String) -> Result<()> {
        if let Some(parent_dir) = path.parent() {
            let _ = std::fs::create_dir_all(parent_dir)?;
        }

        let _ = fs::write(
            &path,
            format!(
                "{}\n\n{addition}",
                toml::to_string(&AllConfig {
                    vizier: self.clone(),
                })?
            ),
        )?;

        Ok(())
    }
}
