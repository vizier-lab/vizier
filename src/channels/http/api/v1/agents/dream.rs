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
    schema::{
        DreamStatus,
        dream_journal::{self, DreamCycle, DreamJournalEntry, DreamNights},
    },
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
        .route("/nights", get(dream_nights))
        .route("/cycles", get(dream_cycles))
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

/// `+07:00`, `-05:30`, `Z` or `+0700` as minutes east of UTC; anything else is UTC.
pub(crate) fn parse_offset(tz: Option<&str>) -> i32 {
    let Some(tz) = tz.map(str::trim).filter(|t| !t.is_empty() && *t != "Z") else {
        return 0;
    };
    let sign = if tz.starts_with('-') { -1 } else { 1 };
    let digits: String = tz.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 2 {
        return 0;
    }
    let hours: i32 = digits[..2].parse().unwrap_or(0);
    let minutes: i32 = digits.get(2..4).and_then(|m| m.parse().ok()).unwrap_or(0);
    if hours > 14 || minutes > 59 {
        return 0;
    }
    sign * (hours * 60 + minutes)
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct NightsQuery {
    /// 1–90; default 30.
    pub days: Option<u32>,
    /// The person's UTC offset, e.g. `+07:00`; default UTC.
    pub tz: Option<String>,
}

async fn editable_agent(
    state: &HTTPState,
    agent_id: &str,
    user: &crate::channels::http::auth::AuthenticatedUser,
) -> Result<(), (StatusCode, String)> {
    match state.storage.get_agent(agent_id).await {
        Ok(Some(config)) if user_can_edit_agent(user, &config) => Ok(()),
        Ok(Some(_)) => Err((StatusCode::FORBIDDEN, "Access denied".into())),
        Ok(None) => Err((StatusCode::NOT_FOUND, format!("agent {agent_id} not found"))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// The cycle in progress, from the stored status.
async fn running_cycle(state: &HTTPState, agent_id: &str) -> Option<(String, DateTime<Utc>)> {
    match state.storage.get_dream_status(agent_id).await.ok().flatten()? {
        DreamStatus::Extracting { cycle_id, started_at, .. } | DreamStatus::Consolidating { cycle_id, started_at } => {
            Some((cycle_id, started_at))
        }
        DreamStatus::Idle => None,
    }
}

/// What each cycle in `cycles` cost, from the usage rows its conversations wrote.
async fn price_cycles(state: &HTTPState, agent_id: &str, cycles: &mut [DreamCycle]) {
    use crate::{
        schema::usage::UsageActivity,
        storage::usage::{UsageGrouping, UsageQuery, UsageStorage},
        usage::Pricer,
    };
    let Some(from) = cycles.iter().map(|c| c.started_at).min() else { return };
    let query = UsageQuery {
        agent_id: agent_id.to_string(),
        start: from - chrono::Duration::hours(1),
        end: Utc::now() + chrono::Duration::minutes(1),
        activity: Some(UsageActivity::Dreams),
        conversation: None,
    };
    let Ok(groups) = state.storage.usage_groups(&query, UsageGrouping::Conversation).await else { return };
    let mut pricer = Pricer::new(&state.catalogue);
    for cycle in cycles {
        let mut total: Option<f64> = None;
        for g in groups.iter().filter(|g| g.topic.as_deref() == Some(cycle.cycle_id.as_str())) {
            if let Some(c) = pricer.cost(&g.provider, &g.model, &g.tokens) {
                *total.get_or_insert(0.0) += c;
            }
        }
        cycle.cost_usd = total;
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/dream/nights",
    params(("agent_id" = String, Path, description = "Agent ID"), NightsQuery),
    responses(
        (status = 200, description = "One entry per night, oldest first, with what each dream changed", body = APIResponse<DreamNights>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn dream_nights(
    Path(agent_id): Path<String>,
    Query(query): Query<NightsQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<DreamNights> {
    if let Err((status, message)) = editable_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }
    let days = query.days.unwrap_or(30).clamp(1, 90);
    let offset = parse_offset(query.tz.as_deref());
    let now = Utc::now();
    // A day of slack either side, so a night near the edge of the range is whole.
    let since = now - chrono::Duration::days(days as i64 + 1);

    let stubs = state.storage.list_dream_entry_stubs(agent_id.clone(), since).await.unwrap_or_default();
    let revisions = state.storage.list_dream_revisions(agent_id.clone(), since).await.unwrap_or_default();
    let running = running_cycle(&state, &agent_id).await;
    let (cycles, unattributed) =
        dream_journal::assemble_cycles(&stubs, &revisions, running.as_ref().map(|(id, at)| (id.as_str(), *at)));

    api_response(
        StatusCode::OK,
        DreamNights {
            nights: dream_journal::build_nights(&cycles, days, dream_journal::night_of(now, offset), offset),
            unattributed,
        },
    )
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct CyclesQuery {
    /// The `next` of the previous page: an RFC 3339 start time; cycles older than it follow.
    pub before: Option<DateTime<Utc>>,
    /// 1–50; default 20.
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct DreamCycles {
    pub cycles: Vec<DreamCycle>,
    /// Pass as `before` for the next page; `null` at the end.
    pub next: Option<DateTime<Utc>>,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/dream/cycles",
    params(("agent_id" = String, Path, description = "Agent ID"), CyclesQuery),
    responses(
        (status = 200, description = "Dream cycles, newest first, with their changes and cost", body = APIResponse<DreamCycles>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn dream_cycles(
    Path(agent_id): Path<String>,
    Query(query): Query<CyclesQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<DreamCycles> {
    if let Err((status, message)) = editable_agent(&state, &agent_id, &user).await {
        return err_response(status, message);
    }
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let since = DateTime::<Utc>::UNIX_EPOCH;

    let stubs = state.storage.list_dream_entry_stubs(agent_id.clone(), since).await.unwrap_or_default();
    let revisions = state.storage.list_dream_revisions(agent_id.clone(), since).await.unwrap_or_default();
    let running = running_cycle(&state, &agent_id).await;
    let (mut cycles, _) =
        dream_journal::assemble_cycles(&stubs, &revisions, running.as_ref().map(|(id, at)| (id.as_str(), *at)));

    if let Some(before) = query.before {
        cycles.retain(|c| c.started_at < before);
    }
    let next = (cycles.len() > limit).then(|| cycles[limit - 1].started_at);
    cycles.truncate(limit);
    price_cycles(&state, &agent_id, &mut cycles).await;

    api_response(StatusCode::OK, DreamCycles { cycles, next })
}

#[cfg(test)]
mod offset_tests {
    use super::parse_offset;

    #[test]
    fn reads_the_offsets_a_browser_can_send() {
        assert_eq!(parse_offset(Some("+07:00")), 420);
        assert_eq!(parse_offset(Some("-05:30")), -330);
        assert_eq!(parse_offset(Some("+0100")), 60);
        assert_eq!(parse_offset(Some("Z")), 0);
        assert_eq!(parse_offset(None), 0);
        assert_eq!(parse_offset(Some("nonsense")), 0);
        assert_eq!(parse_offset(Some("+99:00")), 0);
    }
}
