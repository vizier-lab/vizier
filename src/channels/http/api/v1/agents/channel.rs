use axum::{
    Extension, Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    routing::{any, delete, get},
};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::{
    channels::{
        http::{
            models::{
                self,
                response::{APIResponse, api_response, err_response},
            },
            rate_limit::RateLimiter,
            state::HTTPState,
        },
        reactions::{self, ApplyOutcome, ReactionChange, ReactionKind, ReactionTarget},
    },
    agents::client::{
        ClientAnswer, ClientBinding, ClientConnection, DeferredClaims, declare, deliver_deferred,
    },
    schema::{
        ClientEvent, ReactionAction, SessionHistory, SessionHistoryContent, TopicId,
        VizierAttachmentContent, VizierChannelId, VizierRequest, VizierRequestContent,
        VizierSession, VizierSessionDetail,
    },
    storage::{
        agent::AgentStorage, background_job::BackgroundJobStorage,
        client_call::ClientCallStorage, history::HistoryStorage,
        reaction::{ReactionStorage, Reactor},
        session::SessionStorage,
    },
    transport::{SessionEvent, SessionFrame, VizierTransport},
};

use super::user_can_view_agent;
use crate::channels::http::models::page::Page;
use crate::schema::session_file::SessionFileRecord;
use crate::storage::session_file::SessionFileStorage;

/// The channel a read-only route means by `channel_id`: a Discord or Telegram conversation by its
/// slug (`discord__123`, `telegram__-9`, which the conversation picker lists), anything else a
/// WebUI channel of the caller's own. Routes that write never use this: nobody chats on Discord
/// through the WebUI.
fn readable_channel(username: &str, channel_id: &str) -> (VizierChannelId, bool) {
    if let Some(id) = channel_id.strip_prefix("discord__").and_then(|id| id.parse::<u64>().ok()) {
        return (VizierChannelId::DiscordChanel(id), true);
    }
    if let Some(id) = channel_id.strip_prefix("telegram__").and_then(|id| id.parse::<i64>().ok()) {
        return (VizierChannelId::TelegramChannel(id), true);
    }
    (VizierChannelId::HTTP(username.to_string(), channel_id.to_string()), false)
}

/// The session a read-only route opens. Discord and Telegram conversations have no topic.
fn readable_session(agent_id: String, username: &str, channel_id: &str, topic_id: TopicId) -> VizierSession {
    let (channel, platform) = readable_channel(username, channel_id);
    VizierSession(agent_id, channel, if platform { None } else { Some(topic_id) })
}

pub fn channel() -> Router<HTTPState> {
    Router::new()
        .route("/{channel_id}/topics", get(list_topics))
        .route("/{channel_id}/topic/{topic_id}/chat", any(chat))
        .route(
            "/{channel_id}/topic/{topic_id}/history",
            get(get_topic_history),
        )
        .route("/{channel_id}/topic/{topic_id}/files", get(list_topic_files))
        .route(
            "/{channel_id}/topic/{topic_id}/detail",
            get(get_topic_detail),
        )
        .route("/{channel_id}/topic/{topic_id}", delete(delete_topic))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct HistoryQuery {
    pub before: Option<chrono::DateTime<Utc>>,
    /// The ordering position of the oldest entry of the previous page, completing the
    /// `before` cursor so a group of entries sharing a millisecond cannot be split across a
    /// page boundary. Ignored without `before`; omitting it keeps the pre-`seq` behaviour.
    pub before_seq: Option<i64>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct TopicEntry {
    pub topic_id: String,
    pub title: String,
    pub agent_id: String,
    pub channel: String,
    pub is_thinking: bool,
    /// Background jobs launched from this topic that are still in flight.
    pub running_jobs: usize,
}

impl From<VizierSessionDetail> for TopicEntry {
    fn from(detail: VizierSessionDetail) -> Self {
        Self {
            topic_id: detail.topic.unwrap_or_default(),
            title: detail.title,
            agent_id: detail.agent_id,
            channel: format!("{:?}", detail.channel),
            is_thinking: detail.is_thinking,
            running_jobs: 0,
        }
    }
}

#[derive(Debug, Deserialize)]
struct WebSocketReactionMessage {
    reaction: WebSocketReactionPayload,
}

#[derive(Debug, Deserialize)]
struct WebSocketReactionPayload {
    message_uid: String,
    emoji: String,
    action: ReactionAction,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/history",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID")
    ),
    request_body = HistoryQuery,
    responses(
        (status = 200, description = "Topic history", body = APIResponse<Vec<SessionHistory>>),
        (status = 404, description = "Agent or topic not found", body = APIResponse<String>)
    )
)]
pub async fn get_topic_history(
    Path((agent_id, channel_id, topic_id)): Path<(String, String, TopicId)>,
    Query(params): Query<HistoryQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<Vec<SessionHistory>> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let session = readable_session(agent_id, &user.username, &channel_id, topic_id);

    let response = state
        .storage
        .list_session_history(session, params.before, params.before_seq, params.limit)
        .await;

    // Every entry kind is returned, `AssistantMessage` included: the intermediate narration
    // an agent writes alongside its tool calls is part of the trail the WebUI renders, and
    // dropping it here is what used to make it invisible.
    match response {
        Ok(history) => api_response(StatusCode::OK, history),
        Err(_) => err_response(StatusCode::NOT_FOUND, "Not found".into()),
    }
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct FilesQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/files",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID"),
        FilesQuery
    ),
    responses(
        (status = 200, description = "Files saved to the topic, newest first", body = APIResponse<Page<SessionFileRecord>>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>)
    )
)]
pub async fn list_topic_files(
    Path((agent_id, channel_id, topic_id)): Path<(String, String, TopicId)>,
    Query(params): Query<FilesQuery>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<Page<SessionFileRecord>> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let session = readable_session(agent_id, &user.username, &channel_id, topic_id);
    let mut files = match state.storage.list_session_files(&session).await {
        Ok(files) => files,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    files.sort_by(|a, b| b.added_at.cmp(&a.added_at));
    let total = files.len();
    let items = files
        .into_iter()
        .skip(params.offset.unwrap_or(0))
        .take(params.limit.unwrap_or(100).clamp(1, 100))
        .collect();
    api_response(StatusCode::OK, Page { items, total, counts: Default::default() })
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/channel/{channel_id}/topics",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID")
    ),
    responses(
        (status = 200, description = "List of topics", body = APIResponse<Vec<TopicEntry>>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn list_topics(
    Path((agent_id, channel_id)): Path<(String, String)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<Vec<TopicEntry>> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let channel = VizierChannelId::HTTP(user.username, channel_id);

    let response = state
        .storage
        .get_session_list(agent_id.clone(), Some(channel.clone()))
        .await;

    // One count query for the whole list; a failure only loses the badge.
    let running_jobs = state
        .storage
        .count_running_background_jobs(agent_id, channel)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("failed to count running background jobs: {}", e);
            Default::default()
        });

    if response.is_err() {
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch topics".into(),
        );
    }

    let list = response
        .unwrap()
        .into_iter()
        .map(|detail| {
            let running = running_jobs.get(&detail.topic).copied().unwrap_or(0);
            let mut entry = TopicEntry::from(detail);
            entry.running_jobs = running;
            entry
        })
        .collect();

    api_response(StatusCode::OK, list)
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/detail",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID")
    ),
    responses(
        (status = 200, description = "Topic detail", body = APIResponse<TopicEntry>),
        (status = 404, description = "Agent or topic not found", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn get_topic_detail(
    Path((agent_id, channel_id, topic_id)): Path<(String, String, TopicId)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<TopicEntry> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let (channel, platform) = readable_channel(&user.username, &channel_id);

    match state
        .storage
        .get_session_detail_by_topic(agent_id, channel, if platform { None } else { Some(topic_id) })
        .await
    {
        Ok(Some(detail)) => api_response(StatusCode::OK, TopicEntry::from(detail)),
        Ok(None) => err_response(StatusCode::NOT_FOUND, "Topic not found".into()),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    delete,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID")
    ),
    responses(
        (status = 200, description = "Topic deleted", body = APIResponse<String>),
        (status = 404, description = "Agent or topic not found", body = APIResponse<String>)
    )
)]
pub async fn delete_topic(
    Path((agent_id, channel_id, topic_id)): Path<(String, String, TopicId)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> models::response::Response<String> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found")),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let channel = VizierChannelId::HTTP(user.username, channel_id);

    let response = state
        .storage
        .delete_session(agent_id, channel, topic_id)
        .await;

    if response.is_err() {
        return err_response(StatusCode::NOT_FOUND, "Topic not found".into());
    }

    api_response(StatusCode::OK, "Topic deleted".into())
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/chat",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID")
    ),
    responses(
        (status = 101, description = "WebSocket connection established"),
        (status = 404, description = "Agent not found")
    )
)]
pub async fn chat(
    Path((agent_id, channel_id, topic_id)): Path<(String, String, TopicId)>,
    ws: WebSocketUpgrade,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::channels::http::auth::AuthenticatedUser>,
) -> axum::response::Response {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            return axum::response::Response::builder()
                .status(404)
                .body(axum::body::Body::empty())
                .unwrap();
        }
        Err(e) => {
            tracing::error!("failed to get agent: {}", e);
            return axum::response::Response::builder()
                .status(500)
                .body(axum::body::Body::empty())
                .unwrap();
        }
    };

    if !user_can_view_agent(&user, &config) {
        return axum::response::Response::builder()
            .status(403)
            .body(axum::body::Body::empty())
            .unwrap();
    }

    let transport = state.transport.clone();
    let session = VizierSession(agent_id, VizierChannelId::HTTP(user.username, channel_id), Some(topic_id));
    let ws_idle_timeout_secs = state
        .config
        .channels
        .http
        .as_ref()
        .map(|c| c.ws_idle_timeout_secs)
        .unwrap_or(300);

    let limits = ClientLimits {
        events: state.client_events.clone(),
        state_saves: state.client_state_saves.clone(),
        deferred: state.deferred_client_calls.clone(),
    };
    ws.on_upgrade(move |socket| handle_socket(socket, session, transport, state.storage.clone(), state.config.workspace.clone(), ws_idle_timeout_secs, limits))
}

/// The limits on what a client may send (`specs/019-client-tools` research R15), shared by
/// every socket so they hold across a person's tabs.
#[derive(Clone)]
pub struct ClientLimits {
    /// Client events, keyed by session slug.
    pub events: std::sync::Arc<RateLimiter>,
    /// Saved state, keyed by call id.
    pub state_saves: std::sync::Arc<RateLimiter>,
    /// Deferred calls being delivered by some socket right now.
    pub deferred: std::sync::Arc<DeferredClaims>,
}

/// Deliver deferred calls to this socket in the background (research R18), so a slow answer
/// never holds up the socket's own loop.
fn spawn_delivery(
    storage: &std::sync::Arc<crate::storage::VizierStorage>,
    transport: &VizierTransport,
    session: &VizierSession,
    connection: &std::sync::Arc<ClientConnection>,
    claims: &std::sync::Arc<DeferredClaims>,
    call_ids: Option<Vec<String>>,
) {
    let (storage, transport, session, connection, claims) =
        (storage.clone(), transport.clone(), session.clone(), connection.clone(), claims.clone());
    tokio::spawn(async move {
        let call_ids = match call_ids {
            Some(ids) => ids,
            None => match storage.deferred_client_calls(&session).await {
                Ok(ids) => ids,
                Err(e) => {
                    tracing::warn!("failed to list deferred client tool calls: {e}");
                    return;
                }
            },
        };
        futures::future::join_all(call_ids.iter().map(|call_id| {
            deliver_deferred(&storage, &transport, &session, &connection, &claims, call_id)
        }))
        .await;
    });
}

const MAX_RESULT_BYTES: usize = 1024 * 1024;
const MAX_STATE_BYTES: usize = 64 * 1024;
const MAX_EVENT_TEXT_BYTES: usize = 4 * 1024;
const MAX_EVENT_DATA_BYTES: usize = 64 * 1024;

fn json_len(value: &serde_json::Value) -> usize {
    serde_json::to_string(value).map(|s| s.len()).unwrap_or(usize::MAX)
}

/// A `client_error` frame for this socket only.
fn client_error(code: &str, call_id: Option<&str>, message: impl Into<String>) -> serde_json::Value {
    let mut error = serde_json::json!({ "code": code, "message": message.into() });
    if let Some(call_id) = call_id {
        error["call_id"] = serde_json::json!(call_id);
    }
    serde_json::json!({ "client_error": error })
}

/// Whether `uid` is a client tool call in `session`: the scope of saved state and of a client
/// event's `source_call_id`, so a person cannot reach another's conversation by guessing ids.
async fn is_client_call_in(
    storage: &crate::storage::VizierStorage,
    session: &VizierSession,
    uid: &str,
) -> anyhow::Result<bool> {
    Ok(matches!(
        storage.get_history_entry(uid).await?,
        Some(entry) if &entry.vizier_session == session
            && matches!(entry.content, SessionHistoryContent::ClientToolCall { .. })
    ))
}

/// A `client_tool_result` (protocol §4): `Err` is the `client_error` frame to send back.
fn client_tool_result(
    connection: &ClientConnection,
    payload: &serde_json::Value,
) -> Result<(), serde_json::Value> {
    let Some(call_id) = payload.get("call_id").and_then(|id| id.as_str()) else {
        return Err(client_error("invalid_frame", None, "client_tool_result needs a call_id"));
    };
    let answer = if let Some(ok) = payload.get("ok") {
        if json_len(ok) > MAX_RESULT_BYTES {
            return Err(client_error(
                "result_too_large",
                Some(call_id),
                format!("the result is larger than {MAX_RESULT_BYTES} bytes"),
            ));
        }
        ClientAnswer::Ok(ok.clone())
    } else if let Some(error) = payload.get("error") {
        ClientAnswer::Err(match error {
            serde_json::Value::String(message) => message.clone(),
            other => other.to_string(),
        })
    } else {
        return Err(client_error("invalid_frame", Some(call_id), "client_tool_result needs ok or error"));
    };
    if connection.answer(call_id, answer) {
        Ok(())
    } else {
        Err(client_error(
            "not_pending",
            Some(call_id),
            "this call is not waiting for an answer on this connection",
        ))
    }
}

/// A `client_call_state` save (protocol §6): stored, then echoed to every socket on the
/// session. `Err` is the `client_error` frame to send back; the stored state is untouched.
async fn client_call_state(
    storage: &crate::storage::VizierStorage,
    transport: &VizierTransport,
    session: &VizierSession,
    limits: &ClientLimits,
    payload: &serde_json::Value,
) -> Result<(), serde_json::Value> {
    let Some(call_id) = payload.get("call_id").and_then(|id| id.as_str()) else {
        return Err(client_error("invalid_frame", None, "client_call_state needs a call_id"));
    };
    let state = payload.get("state").cloned().unwrap_or(serde_json::Value::Null);

    match is_client_call_in(storage, session, call_id).await {
        Ok(true) => {}
        Ok(false) => {
            return Err(client_error(
                "not_found",
                Some(call_id),
                "no client tool call with this id in this conversation",
            ));
        }
        Err(err) => {
            tracing::warn!("failed to look up client tool call {call_id}: {err}");
            return Err(client_error("state_not_saved", Some(call_id), "state not saved"));
        }
    }
    if json_len(&state) > MAX_STATE_BYTES {
        return Err(client_error(
            "state_too_large",
            Some(call_id),
            format!("state is larger than {MAX_STATE_BYTES} bytes"),
        ));
    }
    if !limits.state_saves.check(call_id) {
        return Err(client_error(
            "rate_limited",
            Some(call_id),
            "too many state saves for this call; try again shortly",
        ));
    }
    if let Err(err) = storage.save_client_call_state(call_id, &state).await {
        tracing::warn!("failed to save state of client tool call {call_id}: {err}");
        return Err(client_error("state_not_saved", Some(call_id), "state not saved"));
    }
    transport.publish_session_event(SessionEvent {
        session: session.clone(),
        frame: SessionFrame::ClientCallState {
            call_id: call_id.to_string(),
            state,
        },
    });
    Ok(())
}

/// Check a client event (protocol §5) and drop a `source_call_id` that is not a client tool
/// call in this conversation. `Err` is the `client_error` frame refusing it.
async fn check_client_event(
    storage: &crate::storage::VizierStorage,
    session: &VizierSession,
    limits: &ClientLimits,
    event: &mut ClientEvent,
) -> Result<(), serde_json::Value> {
    if event.text.trim().is_empty() || event.text.len() > MAX_EVENT_TEXT_BYTES {
        return Err(client_error(
            "invalid_client_event",
            None,
            format!("text must be 1 to {MAX_EVENT_TEXT_BYTES} bytes"),
        ));
    }
    if event.data.as_ref().is_some_and(|data| json_len(data) > MAX_EVENT_DATA_BYTES) {
        return Err(client_error(
            "invalid_client_event",
            None,
            format!("data must be at most {MAX_EVENT_DATA_BYTES} bytes"),
        ));
    }
    if !limits.events.check(&session.to_slug()) {
        return Err(client_error(
            "rate_limited",
            None,
            "too many client events in this conversation; try again shortly",
        ));
    }
    if let Some(source) = &event.source_call_id
        && !is_client_call_in(storage, session, source).await.unwrap_or(false)
    {
        event.source_call_id = None;
    }
    Ok(())
}

/// Move a request's checked declarations into its binding to `connection`. Only a request
/// that starts a chat turn takes client tools; on any other they are dropped unread.
fn bind_client_tools(
    request: &mut VizierRequest,
    connection: &std::sync::Arc<ClientConnection>,
) -> Result<(), serde_json::Value> {
    let tools = std::mem::take(&mut request.client_tools);
    let categories = std::mem::take(&mut request.client_categories);
    request.client = None;
    let takes_tools = matches!(
        request.content,
        VizierRequestContent::Chat(_) | VizierRequestContent::AudioChat(..) | VizierRequestContent::ClientEvent(_)
    );
    if tools.is_empty() || !takes_tools {
        return Ok(());
    }
    declare::validate(&tools, &categories).map_err(|err| err.frame())?;
    request.client = Some(ClientBinding {
        connection: connection.clone(),
        tools,
        categories,
    });
    Ok(())
}

/// A reaction from this socket, applied through the shared ingest path. The new set reaches
/// every socket on the session (this one included, as its acknowledgement) through the
/// session-event broadcast; an `Err` is the message for a `reaction_error` to this socket only.
async fn webui_reaction(
    storage: &crate::storage::VizierStorage,
    transport: &VizierTransport,
    curr_session: &VizierSession,
    payload: &WebSocketReactionPayload,
) -> Result<(), String> {
    // Reacting is scoped to the socket's own conversation, so one person cannot reach into
    // another's by guessing a uid.
    match storage.get_history_entry(&payload.message_uid).await {
        Ok(Some(entry)) if &entry.vizier_session == curr_session => {}
        Ok(_) => return Err("message not found in this conversation".to_string()),
        Err(err) => {
            tracing::warn!("failed to look up reaction target {}: {err}", payload.message_uid);
            return Err("reaction not saved".to_string());
        }
    }

    let username = match &curr_session.1 {
        VizierChannelId::HTTP(user, _) => user.clone(),
        _ => "unknown".to_string(),
    };
    let kind = match payload.action {
        ReactionAction::Added => ReactionKind::Add(payload.emoji.clone()),
        ReactionAction::Removed => ReactionKind::Remove(payload.emoji.clone()),
    };
    let change = ReactionChange {
        target: ReactionTarget::History(payload.message_uid.clone()),
        reactor: Reactor { id: username, name: None },
        kind,
    };

    match reactions::apply(storage, transport, change).await {
        Ok(ApplyOutcome::Applied) => Ok(()),
        Ok(ApplyOutcome::Ignored(reason)) => Err(reason.to_string()),
        Err(err) => {
            tracing::warn!("failed to save reaction on {}: {err}", payload.message_uid);
            Err("reaction not saved".to_string())
        }
    }
}

pub async fn handle_socket(
    socket: WebSocket,
    curr_session: VizierSession,
    transport: VizierTransport,
    storage: std::sync::Arc<crate::storage::VizierStorage>,
    workspace: String,
    ws_idle_timeout_secs: u64,
    limits: ClientLimits,
) {
    let (mut writer, mut reader) = socket.split();
    let idle_timeout = Duration::from_secs(ws_idle_timeout_secs);

    let (write_tx, mut write_rx) = mpsc::channel::<Message>(64);

    // Writer task: handles all outgoing messages
    let write_handle = tokio::spawn(async move {
        while let Some(msg) = write_rx.recv().await {
            if writer.send(msg).await.is_err() {
                break;
            }
        }
    });

    // This socket's end of the client tool calls made to it. Closed when the socket ends,
    // which fails every call still waiting on it with `client_disconnected`.
    let connection = ClientConnection::new(write_tx.clone());
    let send_frame = |frame: serde_json::Value| {
        let write_tx = write_tx.clone();
        async move {
            let _ = write_tx.send(Message::Text(frame.to_string().into())).await;
        }
    };

    // Calls deferred while no client was here are this socket's to deliver first.
    spawn_delivery(&storage, &transport, &curr_session, &connection, &limits.deferred, None);

    // Background job changes, reactions, and the frames of every turn on this session: the
    // ones a background report woke and the ones a person started from any socket, this one
    // included. A turn's frames are published only there, so nothing arrives twice.
    let mut session_events = transport.subscribe_session_events();
    let mut session_events_open = true;

    let mut ping_interval = tokio::time::interval(Duration::from_secs(30));
    let mut last_activity = tokio::time::Instant::now();
    let mut idle_deadline = tokio::time::sleep(idle_timeout);
    tokio::pin!(idle_deadline);

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                let _ = write_tx.send(Message::Ping(vec![].into())).await;
            }
            event = session_events.recv(), if session_events_open => {
                match event {
                    Ok(ev) if ev.session == curr_session => {
                        let json = match ev.frame {
                            SessionFrame::Response(response) => serde_json::to_string(&response).ok(),
                            SessionFrame::Job(snapshot) => {
                                Some(serde_json::json!({ "background_job": snapshot }).to_string())
                            }
                            SessionFrame::Reactions { history_uid, reactions } => Some(
                                serde_json::json!({
                                    "reactions": { "message_uid": history_uid, "reactions": reactions }
                                })
                                .to_string(),
                            ),
                            // The declaring socket already has the call itself, sent directly.
                            SessionFrame::ClientToolCall { connection_id, call } => (connection_id
                                != connection.id)
                                .then(|| call.wire(false).to_string()),
                            SessionFrame::ClientCallOutcome { call_id, outcome } => Some(
                                serde_json::json!({
                                    "client_tool_outcome": { "call_id": call_id, "outcome": outcome }
                                })
                                .to_string(),
                            ),
                            SessionFrame::ClientCallDeferred { call_id } => {
                                spawn_delivery(&storage, &transport, &curr_session, &connection, &limits.deferred, Some(vec![call_id]));
                                None
                            }
                            SessionFrame::HistoryEntry(entry) => {
                                serde_json::to_string(&serde_json::json!({ "history_entry": entry })).ok()
                            }
                            SessionFrame::File(file) => {
                                Some(serde_json::json!({ "session_file": { "file": file } }).to_string())
                            }
                            SessionFrame::ClientCallState { call_id, state } => Some(
                                serde_json::json!({
                                    "client_call_state": { "call_id": call_id, "state": state }
                                })
                                .to_string(),
                            ),
                        };
                        if let Some(json) = json {
                            let _ = write_tx.send(Message::Text(json.into())).await;
                        }
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        // The client re-reads the job list on reconnect, so a dropped frame
                        // never leaves the tray wrong for good.
                        tracing::warn!("websocket for {:?} lagged, {} session event(s) dropped", curr_session, skipped);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        session_events_open = false;
                    }
                }
            }
            msg = reader.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text_str = text.to_string();
                        let frame = serde_json::from_str::<serde_json::Value>(&text_str).ok();
                        let client_frame = |key: &str| frame.as_ref().and_then(|f| f.get(key)).filter(|p| p.is_object());

                        if let Some(payload) = client_frame("client_tool_result") {
                            if let Err(error) = client_tool_result(&connection, payload) {
                                send_frame(error).await;
                            }
                        } else if let Some(payload) = client_frame("client_call_state") {
                            if let Err(error) = client_call_state(&storage, &transport, &curr_session, &limits, payload).await {
                                send_frame(error).await;
                            }
                        } else if let Ok(reaction_msg) = serde_json::from_str::<WebSocketReactionMessage>(&text_str) {
                            let payload = reaction_msg.reaction;
                            if let Err(message) = webui_reaction(&storage, &transport, &curr_session, &payload).await {
                                let frame = serde_json::json!({
                                    "reaction_error": { "message_uid": payload.message_uid, "message": message }
                                });
                                let _ = write_tx.send(Message::Text(frame.to_string().into())).await;
                            }
                        } else if let Ok(request) = serde_json::from_str::<VizierRequest>(&text_str) {
                            let mut request = request.clone();
                            // A person's message starts a fresh chain; the depth is never
                            // the client's to set.
                            request.background_depth = 0;

                            // Refused before anything is sent, so no turn starts (FR-002).
                            if let Err(error) = bind_client_tools(&mut request, &connection) {
                                send_frame(error).await;
                                continue;
                            }
                            if let VizierRequestContent::ClientEvent(event) = &mut request.content
                                && let Err(error) = check_client_event(&storage, &curr_session, &limits, event).await
                            {
                                send_frame(error).await;
                                continue;
                            }
                            for attachment in request.attachments.iter_mut() {
                                match transport.send_file_resolve(attachment.clone()).await {
                                    Ok(content) => {
                                        match transport.send_file_upload(attachment.filename.clone(), content).await {
                                            Ok(file_record) => {
                                                attachment.content = VizierAttachmentContent::Local(file_record.url);
                                            }
                                            Err(e) => {
                                                tracing::error!("failed to upload resolved attachment: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("failed to resolve attachment: {}", e);
                                    }
                                }
                            }

                            let (response_tx, response_rx) = flume::unbounded();
                            let session = curr_session.clone();
                            if let Err(err) = transport.send_request(session.clone(), request, Some(response_tx)).await {
                                tracing::error!("failed to send request: {}", err);
                                continue;
                            }

                            // The turn's frames go to every socket on the session, not to
                            // this one: a person who leaves the page mid-turn and comes back
                            // (or has it open in another tab) still sees the rest of it. This
                            // socket receives them through `session_events` like the others.
                            let transport = transport.clone();
                            tokio::spawn(async move {
                                while let Ok(response) = response_rx.recv_async().await {
                                    transport.publish_session_event(SessionEvent {
                                        session: session.clone(),
                                        frame: SessionFrame::Response(response),
                                    });
                                }
                            });
                        } else {
                            tracing::debug!("unrecognized WebSocket message: {}", &text_str[..text_str.len().min(200)]);
                        }
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Pong(_))) => {
                        last_activity = tokio::time::Instant::now();
                        idle_deadline.as_mut().reset(last_activity + idle_timeout);
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = write_tx.send(Message::Pong(data)).await;
                        last_activity = tokio::time::Instant::now();
                        idle_deadline.as_mut().reset(last_activity + idle_timeout);
                    }
                    Some(Ok(Message::Binary(_))) => {
                        last_activity = tokio::time::Instant::now();
                        idle_deadline.as_mut().reset(last_activity + idle_timeout);
                    }
                    Some(Err(e)) => {
                        tracing::warn!("websocket read error: {:?}", e);
                        break;
                    }
                    None => break,
                }
                last_activity = tokio::time::Instant::now();
                idle_deadline.as_mut().reset(last_activity + idle_timeout);
            }
            _ = &mut idle_deadline => {
                tracing::warn!("websocket idle timeout for session {:?}", curr_session);
                let _ = write_tx.send(Message::Close(None)).await;
                break;
            }
        }
    }

    connection.close();
    drop(write_tx);
    let _ = write_handle.await;
}
