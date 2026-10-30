use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct FileRecord {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
    pub url: String,
    pub created_at: DateTime<Utc>,
}
