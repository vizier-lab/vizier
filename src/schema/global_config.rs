use serde::{Deserialize, Serialize};

use crate::config::shell::ShellConfig;

#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema)]
#[serde(tag = "type", content = "data")]
pub enum GlobalConfigValue {
    Shell(ShellConfig),
}

#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema)]
pub struct GlobalConfigEntry {
    pub key: String,
    pub value: GlobalConfigValue,
}
