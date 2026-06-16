use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


use crate::schema::VizierSession;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct SessionFileRecord {
    pub id: String,
    pub session_slug: String,
    pub agent_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
    pub file_id: String,
    pub added_at: DateTime<Utc>,
}
