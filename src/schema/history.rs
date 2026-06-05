use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

use crate::schema::{ReactionEntry, VizierRequest, VizierResponse, VizierSession};

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue, JsonSchema, utoipa::ToSchema)]
pub struct SessionHistory {
    pub uid: String,
    pub vizier_session: VizierSession,
    pub content: SessionHistoryContent,
    #[serde(default)]
    pub reactions: Vec<ReactionEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue, JsonSchema, utoipa::ToSchema)]
pub enum SessionHistoryContent {
    Request(VizierRequest),
    Response(VizierResponse),
}