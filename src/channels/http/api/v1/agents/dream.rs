use axum::{
    Extension, Router,
    extract::{Path, Query, State},
    routing::{get, post},
    Json,
};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    channels::http::{
        models::{
            self,
            response::{api_response, err_response, APIResponse},
        },
        state::HTTPState,
    },
    schema::{DreamStatus, dream_journal::DreamJournalEntry},
    storage::{
        agent::AgentStorage, dream::DreamStorage, dream_journal::DreamJournalStorage,
    },
    transport::DreamCommand,
};

use super::user_can_edit_agent;

pub fn dream() -> Router<HTTPState> {
    Router::new()
        .route("/trigger", post(trigger_dream))
        .route("/status", get(dream_status))
        .route("/journal", get(get_dream_journal))
        .route("/journal/{entry_id}", get(get_dream_entry))
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct DreamStatusResponse {
    pub status: String,
    pub total_sessions: Option<usize>,
    pub completed_sessions: Option<usize>,
    pub last_dream: Option<DateTime<Utc>>,
    pub next_dream: Option<String>,
    pub dream_provider: Option<String>,
    pub dream_model: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct DreamJournalQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/dream/trigger",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Dream cycle started", body = APIResponse<String>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn trigger_dream(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<String> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_edit_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    if !config.dream_enabled {
        return err_response(
            StatusCode::BAD_REQUEST,
            "Dreaming is not enabled for this agent".into(),
        );
    }

    match state
        .transport
        .send_dream_command(DreamCommand {
            agent_id: agent_id.clone(),
            cycle_id: None,
        })
        .await
    {
        Ok(_) => api_response(StatusCode::OK, "Dream cycle started".to_string()),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/dream/status",
    params(
        ("agent_id" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "Dream status", body = APIResponse<DreamStatusResponse>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn dream_status(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<DreamStatusResponse> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_edit_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let status = state
        .storage
        .get_dream_status(&agent_id)
        .await
        .unwrap_or(None)
        .unwrap_or(DreamStatus::Idle);

    let last_dream = state
        .storage
        .get_last_dream_time(&agent_id)
        .await
        .unwrap_or(None);

    // Compute next scheduled dream from cron
    let next_dream = if let Some(ref cron_str) = config.dream_schedule {
        use croner::Cron;
        use std::str::FromStr;
        if let Ok(cron) = Cron::from_str(cron_str) {
            let from = last_dream.unwrap_or(Utc::now());
            cron.find_next_occurrence(&from, true)
                .ok()
                .map(|dt| dt.to_rfc3339())
        } else {
            None
        }
    } else {
        None
    };

    let (status_str, total_sessions, completed_sessions) = match &status {
        DreamStatus::Idle => ("idle".to_string(), None, None),
        DreamStatus::Extracting {
            total_sessions,
            completed_sessions,
            ..
        } => (
            "extracting".to_string(),
            Some(*total_sessions),
            Some(*completed_sessions),
        ),
        DreamStatus::Consolidating { .. } => ("consolidating".to_string(), None, None),
    };

    let response = DreamStatusResponse {
        status: status_str,
        total_sessions,
        completed_sessions,
        last_dream,
        next_dream,
        dream_provider: config.dream_provider.map(|p| format!("{:?}", p)),
        dream_model: config.dream_model,
    };

    api_response(StatusCode::OK, response)
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/dream/journal",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("limit" = Option<usize>, Query, description = "Max entries"),
        ("offset" = Option<usize>, Query, description = "Offset")
    ),
    responses(
        (status = 200, description = "Dream journal entries", body = APIResponse<Vec<DreamJournalEntry>>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn get_dream_journal(
    Path(agent_id): Path<String>,
    Query(query): Query<DreamJournalQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<Vec<DreamJournalEntry>> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_edit_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    match state
        .storage
        .list_dream_entries(agent_id.clone(), query.limit, query.offset)
        .await
    {
        Ok(entries) => api_response(StatusCode::OK, entries),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/dream/journal/{entry_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("entry_id" = String, Path, description = "Entry ID")
    ),
    responses(
        (status = 200, description = "Dream journal entry", body = APIResponse<DreamJournalEntry>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Entry not found", body = APIResponse<String>)
    )
)]
pub async fn get_dream_entry(
    Path((agent_id, entry_id)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<DreamJournalEntry> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_edit_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    match state
        .storage
        .get_dream_entry(agent_id.clone(), entry_id.clone())
        .await
    {
        Ok(Some(entry)) => api_response(StatusCode::OK, entry),
        Ok(None) => err_response(StatusCode::NOT_FOUND, format!("entry {entry_id} not found")),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}
