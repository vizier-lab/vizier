use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

use crate::schema::VizierSession;

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
pub struct ContextFileRecord {
    pub id: String,
    pub session_slug: String,
    pub agent_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
    pub file_id: String,
    pub added_at: DateTime<Utc>,
}
