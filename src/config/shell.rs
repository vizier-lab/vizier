use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, utoipa::ToSchema)]
pub struct DockerShellConfig {
    pub image: DockerSourceConfig,
    pub container_name: String,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

impl Default for DockerShellConfig {
    fn default() -> Self {
        Self {
            image: DockerSourceConfig::default(),
            container_name: "vizier".into(),
            env: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, utoipa::ToSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum DockerSourceConfig {
    Pull { name: String },
    Dockerfile { path: String, name: String },
}

impl Default for DockerSourceConfig {
    fn default() -> Self {
        Self::Pull {
            name: "ubuntu:latest".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, utoipa::ToSchema)]
pub struct LocalShellConfig {
    pub path: String,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, utoipa::ToSchema)]
#[serde(tag = "environment", rename_all = "snake_case")]
pub enum ShellConfig {
    Docker(DockerShellConfig),
    Local(LocalShellConfig),
}
