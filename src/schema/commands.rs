use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandRequest {
    Exit,
    Status,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandResponse {
    Ok(String),
    Error(String),
}

impl std::fmt::Display for CommandResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandResponse::Ok(s) => write!(f, "{s}"),
            CommandResponse::Error(s) => write!(f, "{s}"),
        }
    }
}
