use std::{env::current_dir, fs, path::PathBuf, str::FromStr};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub mod embedding;
pub mod provider;
pub mod shell;
pub mod storage;
pub mod tools;

use crate::{
    config::{
        provider::{LlamaCppProviderConfig, OllamaProviderConfig, ProviderConfig},
        storage::StorageConfig,
    },
    constant,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChannelsConfig {
    pub http: Option<HTTPChannelConfig>,
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
    pub providers: ProviderConfig,
    pub storage: StorageConfig,
    pub channels: ChannelsConfig,
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

#[derive(Debug, Clone, Default)]
pub struct RunOverrides {
    pub workspace: Option<PathBuf>,
    pub port: Option<u32>,
    pub storage: Option<StorageConfig>,
    pub workers: Option<usize>,
    pub ws_idle_timeout: Option<u64>,
}

impl VizierConfig {
    pub fn load(path: Option<std::path::PathBuf>) -> Result<Self> {
        let resolved_path: Option<PathBuf> = match path {
            Some(p) => Some(p),
            None => std::env::var("VIZIER_CONFIG")
                .ok()
                .map(PathBuf::from)
                .or_else(default_config_candidate),
        };

        let config_from_file = resolved_path.is_some();
        let (raw_string, parent_path) = if let Some(p) = resolved_path {
            let s = fs::read_to_string(&p)
                .with_context(|| format!("failed to read config file: {}", p.display()))?;
            let parent = if p.parent().map(|p| p.as_os_str().is_empty()).unwrap_or(true) {
                PathBuf::from_str("./").unwrap()
            } else {
                p.parent().unwrap().to_path_buf()
            };
            (s, parent)
        } else {
            tracing::info!("no config file found, using built-in defaults");
            let yaml = serde_yaml::to_string(&AllConfig {
                vizier: Self::default(),
            })?;
            (yaml, current_dir().unwrap())
        };

        let config_string = shellexpand::env(&raw_string)?;
        let mut config = serde_yaml::from_str::<AllConfig>(&config_string)?;

        let workspace = if config_from_file {
            let mut ws = parent_path.clone();
            ws.push(".vizier");
            let _ = fs::create_dir_all(&ws)?;
            ws.to_string_lossy().to_string()
        } else {
            resolve_default_workspace()?
        };
        config.vizier.workspace = workspace;

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

    pub fn apply_overrides(&mut self, overrides: &RunOverrides) {
        if let Some(ref w) = overrides.workspace {
            self.workspace = w.to_string_lossy().to_string();
            let _ = fs::create_dir_all(w);
        }
        if let Some(p) = overrides.port {
            let http = self.channels.http.get_or_insert_with(default_http_channel);
            http.port = p;
        }
        if let Some(ref s) = overrides.storage {
            self.storage = s.clone();
        }
        if let Some(w) = overrides.workers {
            self.worker_threads = w;
        }
        if let Some(t) = overrides.ws_idle_timeout {
            let http = self.channels.http.get_or_insert_with(default_http_channel);
            http.ws_idle_timeout_secs = t;
        }
    }
}

fn default_http_channel() -> HTTPChannelConfig {
    HTTPChannelConfig {
        port: 9999,
        jwt_secret: "${VIZIER_JWT_SECRET}".into(),
        jwt_expiry_hours: 720,
        ws_idle_timeout_secs: 300,
    }
}

fn default_config_candidate() -> Option<PathBuf> {
    let candidate = current_dir().ok()?.join(constant::DEFAULT_CONFIG_PATH);
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

fn resolve_default_workspace() -> Result<String> {
    let data_dir = std::env::var("VIZIER_DATA_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".vizier")))
        .context("could not determine workspace: VIZIER_DATA_DIR unset and $HOME unknown")?;
    fs::create_dir_all(&data_dir)
        .with_context(|| format!("failed to create workspace at {}", data_dir.display()))?;
    Ok(data_dir.to_string_lossy().to_string())
}

impl Default for VizierConfig {
    fn default() -> Self {
        VizierConfig {
            workspace: String::new(),
            storage: StorageConfig::Sqlite,
            providers: ProviderConfig {
                ollama: Some(OllamaProviderConfig::default()),
                deepseek: None,
                openrouter: None,
                anthropic: None,
                openai: None,
                gemini: None,
                mimo: None,
                llama_cpp: Some(LlamaCppProviderConfig::default()),
                elevenlabs: None,
                groq: None,
                mistral: None,
                xai: None,
                perplexity: None,
                moonshot: None,
                zai: None,
                minimax: None,
                together: None,
                cohere: None,
                huggingface: None,
                hyperbolic: None,
                voyageai: None,
                galadriel: None,
                mira: None,
                chatgpt: None,
                copilot: None,
                azure: None,
            },
            channels: ChannelsConfig {
                http: Some(default_http_channel()),
            },
            worker_threads: 4,
        }
    }
}
