use std::{collections::HashMap, env::current_dir, fs, path::PathBuf, str::FromStr};

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub mod embedding;
pub mod provider;
pub mod shell;
pub mod storage;
pub mod tools;

use crate::{
    config::{
        embedding::{EmbeddingConfig, LocalEmbeddingModelVariant},
        provider::{LlamaCppProviderConfig, MistralrsProviderConfig, OllamaProviderConfig, ProviderConfig},
        storage::StorageConfig,
        tools::{BraveSearchConfig, ToolsConfig},
    },
    constant,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChannelsConfig {
    pub discord: Option<HashMap<String, DiscordChannelConfig>>,
    pub http: Option<HTTPChannelConfig>,
    pub telegram: Option<HashMap<String, TelegramChannelConfig>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiscordChannelConfig {
    pub token: String,
}

impl Default for DiscordChannelConfig {
    fn default() -> Self {
        Self {
            token: "${DISCORD_BOT_TOKEN}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TelegramChannelConfig {
    pub token: String,
}

impl Default for TelegramChannelConfig {
    fn default() -> Self {
        Self {
            token: "${TELEGRAM_BOT_TOKEN}".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HTTPChannelConfig {
    pub port: u32,
    pub jwt_secret: String,
    #[serde(default = "default_jwt_expiry")]
    pub jwt_expiry_hours: u64,
    #[serde(default = "default_ws_idle_timeout_secs")]
    pub ws_idle_timeout_secs: u64,
}

fn default_jwt_expiry() -> u64 {
    720 // 30 days
}

fn default_ws_idle_timeout_secs() -> u64 {
    300 // 5 minutes
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VizierConfig {
    #[serde(skip)]
    pub workspace: String,
    pub embedding: Option<EmbeddingConfig>,
    pub providers: ProviderConfig,
    pub storage: StorageConfig,
    pub channels: ChannelsConfig,
    pub tools: ToolsConfig,
    #[serde(default = "default_worker_threads")]
    pub worker_threads: usize,
}

fn default_worker_threads() -> usize {
    4
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
            tracing::warn!(
                "config path not inputed, fallback to {:?}",
                default_path.to_str().unwrap()
            );

            default_path
        });

        tracing::info!("config loaded: {:?}", path.to_str().unwrap());

        let raw_string = std::fs::read_to_string(&path)?;
        let config_string = shellexpand::env(&raw_string)?;
        let mut config = serde_yaml::from_str::<AllConfig>(&config_string)?;

        let parent_path = if path.parent().unwrap().to_string_lossy() == "" {
            PathBuf::from_str("./").unwrap()
        } else {
            path.parent().unwrap().to_path_buf()
        };

        let mut workspace = parent_path.clone();
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
            storage: StorageConfig::Filesystem(storage::DocumentIndexerConfig::InMem),
            providers: ProviderConfig {
                ollama: Some(OllamaProviderConfig::default()),
                deepseek: None,
                openrouter: None,
                anthropic: None,
                openai: None,
                gemini: None,
                mimo: None,
                llama_cpp: Some(LlamaCppProviderConfig::default()),
                mistralrs: Some(MistralrsProviderConfig::default()),
            },
            embedding: Some(EmbeddingConfig::Local {
                model: LocalEmbeddingModelVariant::AllMiniLml6V2,
            }),
            channels: ChannelsConfig {
                discord: Some(
                    [("vizier".to_string(), DiscordChannelConfig::default())]
                        .into_iter()
                        .collect::<HashMap<String, DiscordChannelConfig>>(),
                ),
                http: Some(HTTPChannelConfig {
                    port: 9999,
                    jwt_secret: "${VIZIER_JWT_SECRET}".into(),
                    jwt_expiry_hours: 720,
                    ws_idle_timeout_secs: 300,
                }),
                telegram: None,
            },
            tools: ToolsConfig {
                brave_search: Some(BraveSearchConfig::default()),
            },
            worker_threads: 4,
        }
    }
}
