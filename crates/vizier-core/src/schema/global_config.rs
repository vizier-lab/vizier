use serde::{Deserialize, Serialize};

use crate::config::shell::ShellConfig;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "type", content = "data")]
pub enum GlobalConfigValue {
    Shell(ShellConfig),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GlobalConfigEntry {
    pub key: String,
    pub value: GlobalConfigValue,
}
