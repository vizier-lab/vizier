use std::{collections::HashMap, fs, str::FromStr};

use anyhow::Result;
use config::Config;
use duration_string::DurationString;
use serde::{Deserialize, Serialize};

pub mod agent;
pub mod embedding;
pub mod provider;
pub mod user;

use crate::{
    config::{
        agent::{AgentConfig, AgentConfigs, AgentToolsConfig, MemoryConfig},
        embedding::VizierEmbeddingModel,
        provider::{
            DeepseekProviderConfig, OllamaProviderConfig, OpenRouterProviderConfig, ProviderConfig,
        },
        user::UserConfig,
    },
    constant,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChannelsConfig {
    pub discord: Option<Vec<DiscordChannelConfig>>,
    pub http: Option<HTTPChannelConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiscordChannelConfig {
    pub agent_id: String,
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HTTPChannelConfig {
    pub port: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolsConfig {
    pub dangerously_enable_cli_access: bool,
    pub brave_search: Option<BraveSearchConfig>,
    pub vector_memory: Option<VectorMemoryConfig>,
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
    pub primary_user: UserConfig,
    pub providers: ProviderConfig,
    pub agents: AgentConfigs,
    pub channels: ChannelsConfig,
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
                serde_yaml::to_string(&AllConfig {
                    vizier: self.clone(),
                })?
            ),
        )?;

        Ok(())
    }
}

impl Default for VizierConfig {
    fn default() -> Self {
        VizierConfig {
            workspace: "~/.vizier".into(),
            primary_user: UserConfig { name: "admin".into(), discord_id: "".into(), discord_username: "".into(), alias: vec![] },
            providers: ProviderConfig {
                ollama: Some(OllamaProviderConfig::default()),
                deepseek: Some(DeepseekProviderConfig::default()),
                openrouter: Some(OpenRouterProviderConfig::default()),
            },
            agents: HashMap::from([(
                "primary".into(),
                AgentConfig {
                    name: "Vizier".into(),
                    description: Some("You are a digital steward of the 21st century.\n\n**Your job:** be genuinely helpful to your primary user, with insight, wit, and a refusal to be boring.
                        ".into()),
                    provider: provider::ProviderVariant::ollama,
                    session_ttl: DurationString::from_string("30m".into()).unwrap(),
                    model: "deepseek/deepseek-v3.2".into(),
                    memory: MemoryConfig {
                        session_memory_recall_depth: 30,
                    },
                    turn_depth: 100,
                    tools: AgentToolsConfig::default(),
                    silent_read_initiative_chance: 0.01,
                },
            )]),
            channels: ChannelsConfig {
                discord: Some(vec![DiscordChannelConfig {
                    token: "".into(),
                    agent_id: "primary".into(),
                }]),
                http: Some(HTTPChannelConfig { port: 9999 }),
            },
            tools: ToolsConfig {
                dangerously_enable_cli_access: false,
                brave_search: Some(BraveSearchConfig {
                    api_key: "".into(),
                    safesearch: true,
                }),
                vector_memory: Some(VectorMemoryConfig {
                    model: VizierEmbeddingModel::AllMiniLML6V2,
                }),
            },
        }
    }
}
