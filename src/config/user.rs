use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct UserConfig {
    pub name: String,
    pub discord_id: String,
    pub discord_username: String,
    pub alias: Vec<String>,
}
