use chrono::Utc;
use rig::Embed;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Memory {
    pub slug: String,
    pub title: String,
    pub content: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub embedding: Vec<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: uuid::Uuid,
    pub metadata: serde_json::Value,
    pub sender: String,
    pub from_agent: bool,
    pub channel: String,
    pub content: String,
    pub timestamp: chrono::DateTime<Utc>,
}
