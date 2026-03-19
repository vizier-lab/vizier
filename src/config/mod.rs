use std::{collections::HashMap, env::current_dir, fs, str::FromStr};

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
    pub discord: Option<HashMap<String, DiscordChannelConfig>>,
    pub http: Option<HTTPChannelConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiscordChannelConfig {
    pub token: Option<String>,
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
    pub api_key: Option<String>,
    #[serde(default)]
    pub safesearch: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VectorMemoryConfig {
    pub model: VizierEmbeddingModel,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VizierConfig {
    #[serde(skip)]
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
        let mut default_path = current_dir().unwrap();
        default_path.push(std::path::PathBuf::from_str(constant::DEFAULT_CONFIG_PATH).unwrap());

        let path = path.unwrap_or_else(|| {
            log::warn!(
                "config path not inputed, fallback to {:?}",
                default_path.to_str().unwrap()
            );

            default_path
        });

        let settings = Config::builder()
            .add_source(config::File::from(path.clone()))
            .build()?;

        log::info!("config loaded: {:?}", path.to_str().unwrap());
        let mut config = settings.try_deserialize::<AllConfig>()?;
        let mut workspace = path.parent().unwrap().to_path_buf();
        workspace.push(".vizier");

        let _ = fs::create_dir_all(&workspace)?;
        config.vizier.workspace = workspace.to_str().unwrap().to_string();

        Ok(config.vizier)
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
                "vizier".into(),
                AgentConfig {
                    name: "Vizier".into(),
                    description: Some("digital steward".into()),
                    preamble: Some("You are a digital steward of the 21st century.\n\n**Your job:** be genuinely helpful to your primary user, with insight, wit, and a refusal to be boring.
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
                    show_thinking: None
                },
            )]),
            channels: ChannelsConfig {
                discord: Some([("vizier".to_string(),DiscordChannelConfig {
                    token: Some("".into()),
                })].into_iter().collect::<HashMap<String,DiscordChannelConfig>>()),
                http: Some(HTTPChannelConfig { port: 9999 }),
            },
            tools: ToolsConfig {
                dangerously_enable_cli_access: false,
                brave_search: Some(BraveSearchConfig {
                    api_key: Some("".into()),
                    safesearch: true,
                }),
                vector_memory: Some(VectorMemoryConfig {
                    model: VizierEmbeddingModel::AllMiniLML6V2,
                }),
            },
        }
    }
}
