use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue, JsonSchema, utoipa::ToSchema)]
pub struct Task {
    pub slug: String,
    pub user: String,
    pub agent_id: String,
    pub title: String,
    pub instruction: String,
    pub is_active: bool,
    pub schedule: TaskSchedule,
    pub last_executed_at: Option<DateTime<Utc>>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue, JsonSchema, utoipa::ToSchema)]
pub enum TaskSchedule {
    CronTask(String),
    OneTimeTask(DateTime<Utc>),
}