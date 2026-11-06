use axum::{
    Extension, Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    routing::{any, delete, get, post},
};

use futures::{SinkExt, StreamExt};
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::mpsc;

use vizier_core::{
    agents::client::{
        ClientAnswer, ClientConnection, declare, deliver_deferred,
    },
    channels::reactions::{self, ApplyOutcome, ReactionChange, ReactionKind, ReactionTarget},
    schema::{
        Artifact, ClientEvent, ReactionAction, Requester, SessionHistory, SessionHistoryContent,
        TopicId, VizierAttachmentContent, VizierChannelId, VizierRequest, VizierRequestContent,
        VizierSession, VizierSessionDetail,
    },
    storage::{
        agent::AgentStorage,
        background_job::BackgroundJobStorage,
        client_call::ClientCallStorage,
        history::HistoryStorage,
        reaction::{ReactionStorage, Reactor},
        session::SessionStorage,
    },
    transport::{SessionEvent, SessionFrame, VizierTransport},
};
use crate::{models::{self, response::{APIResponse, api_response, err_response}}, state::HTTPState};

use super::{respond, svc};
use vizier_core::service::topics;
use vizier_core::access::Caller;
use vizier_core::utils::rate_limit::RateLimiter;
use vizier_core::artifacts::access as ArtifactsAccess;
use crate::models::page::Page;
use vizier_core::schema::session_file::SessionFileRecord;
use vizier_core::storage::session_file::SessionFileStorage;

pub use vizier_core::service::topics::{TopicEntry, readable_channel, readable_session};

pub fn channel() -> Router<HTTPState> {
    Router::new()
        .route("/{channel_id}/topics", get(list_topics))
        .route("/{channel_id}/topic/{topic_id}/chat", any(chat))
        .route(
            "/{channel_id}/topic/{topic_id}/history",
            get(get_topic_history),
        )
        .route(
            "/{channel_id}/topic/{topic_id}/files",
            get(list_topic_files).delete(clear_topic_files),
        )
        .route(
            "/{channel_id}/topic/{topic_id}/files/promote",
            post(promote_topic_file),
        )
        .route(
            "/{channel_id}/topic/{topic_id}/detail",
            get(get_topic_detail),
        )
        .route(
            "/{channel_id}/topic/{topic_id}",
            delete(delete_topic).patch(rename_topic),
        )
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct RenameTopicRequest {
    pub title: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct HistoryQuery {
    #[serde(default, deserialize_with = "vizier_core::time::deserialize_query_time")]
    pub before: Option<chrono::DateTime<chrono::Local>>,
    /// The ordering position of the oldest entry of the previous page, completing the
    /// `before` cursor so a group of entries sharing a millisecond cannot be split across a
    /// page boundary. Ignored without `before`; omitting it keeps the pre-`seq` behaviour.
    pub before_seq: Option<i64>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WebSocketReactionMessage {
    pub(crate) reaction: WebSocketReactionPayload,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WebSocketReactionPayload {
    pub(crate) message_uid: String,
    pub(crate) emoji: String,
    pub(crate) action: ReactionAction,
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
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    respond(
        topics::history(
            &svc(&state),
            &user,
            &agent_id,
            &channel_id,
            topic_id,
            params.before,
            params.before_seq,
            params.limit,
        )
        .await,
        StatusCode::OK,
    )
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
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    let page = topics::list_files(
        &svc(&state),
        &user,
        &agent_id,
        &channel_id,
        topic_id,
        params.offset,
        params.limit,
    )
    .await
    .map(|p| Page {
        items: p.items,
        total: p.total,
        counts: Default::default(),
    });
    respond(page, StatusCode::OK)
}

#[utoipa::path(
    delete,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/files",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID")
    ),
    responses(
        (status = 200, description = "How many session files were removed and their stored bytes", body = APIResponse<vizier_core::session_files::ClearResult>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent not found", body = APIResponse<String>),
        (status = 409, description = "A turn is running or queued in this conversation", body = APIResponse<String>)
    )
)]
pub async fn clear_topic_files(
    Path((agent_id, channel_id, topic_id)): Path<(String, String, TopicId)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    respond(
        topics::clear_files(&svc(&state), &user, &agent_id, &channel_id, topic_id).await,
        StatusCode::OK,
    )
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct PromoteFileRequest {
    pub filename: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[utoipa::path(
    post,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/files/promote",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID")
    ),
    request_body = PromoteFileRequest,
    responses(
        (status = 201, description = "The new artifact", body = APIResponse<Artifact>),
        (status = 403, description = "Access denied", body = APIResponse<String>),
        (status = 404, description = "Agent, conversation or file not found", body = APIResponse<String>),
        (status = 409, description = "An artifact with that name already exists", body = APIResponse<String>),
        (status = 422, description = "Invalid name or empty/unreadable source", body = APIResponse<String>)
    )
)]
pub async fn promote_topic_file(
    Path((agent_id, channel_id, topic_id)): Path<(String, String, TopicId)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    axum::Json(body): axum::Json<PromoteFileRequest>,
) -> axum::response::Response {
    respond(
        topics::promote_file(
            &svc(&state),
            &user,
            &agent_id,
            &channel_id,
            topic_id,
            &body.filename,
            body.name.as_deref(),
        )
        .await,
        StatusCode::CREATED,
    )
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
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    respond(
        topics::list(&svc(&state), &user, &agent_id, &channel_id).await,
        StatusCode::OK,
    )
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
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    respond(
        topics::detail(&svc(&state), &user, &agent_id, &channel_id, topic_id).await,
        StatusCode::OK,
    )
}

#[utoipa::path(
    patch,
    path = "/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
        ("topic_id" = String, Path, description = "Topic ID")
    ),
    request_body = RenameTopicRequest,
    responses(
        (status = 200, description = "Topic renamed", body = APIResponse<TopicEntry>),
        (status = 400, description = "The title is empty or too long", body = APIResponse<String>),
        (status = 404, description = "Agent or topic not found", body = APIResponse<String>)
    )
)]
pub async fn rename_topic(
    Path((agent_id, channel_id, topic_id)): Path<(String, String, TopicId)>,
    State(state): State<HTTPState>,
    Extension(user): Extension<crate::auth::Principal>,
    axum::Json(body): axum::Json<RenameTopicRequest>,
) -> axum::response::Response {
    respond(
        topics::rename(&svc(&state), &user, &agent_id, &channel_id, topic_id, &body.title).await,
        StatusCode::OK,
    )
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
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    respond(
        topics::delete(&svc(&state), &user, &agent_id, &channel_id, topic_id).await,
        StatusCode::OK,
    )
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
    Extension(user): Extension<crate::auth::Principal>,
) -> axum::response::Response {
    let (out_tx, out_rx) = flume::unbounded();
    let topic = match crate::topic::TopicConnection::open(&state, &user, &agent_id, &channel_id, &topic_id, out_tx).await {
        Ok(topic) => topic,
        Err(e) => {
            if matches!(e, vizier_core::service::ServiceError::Internal(_)) {
                tracing::error!("failed to open topic: {}", e);
            }
            return axum::response::Response::builder()
                .status(e.status())
                .body(axum::body::Body::empty())
                .unwrap();
        }
    };

    let ws_idle_timeout_secs = state
        .config
        .channels
        .http
        .as_ref()
        .map(|c| c.ws_idle_timeout_secs)
        .unwrap_or(300);

    ws.on_upgrade(move |socket| handle_socket(socket, topic, ws_idle_timeout_secs, out_rx))
}

/// The limits on what a client may send (`specs/019-client-tools` research R15), shared by
/// every socket so they hold across a person's tabs.
#[derive(Clone)]
pub struct ClientLimits {
    /// Client events, keyed by session slug.
    pub events: std::sync::Arc<RateLimiter>,
    /// Saved state, keyed by call id.
    pub state_saves: std::sync::Arc<RateLimiter>,
}

/// Deliver deferred calls to this socket in the background (research R18), so a slow answer
/// never holds up the socket's own loop.
pub(crate) fn spawn_delivery(
    storage: &std::sync::Arc<vizier_core::storage::VizierStorage>,
    transport: &VizierTransport,
    session: &VizierSession,
    connection: &std::sync::Arc<ClientConnection>,
    call_ids: Option<Vec<String>>,
) {
    let (storage, transport, session, connection) = (
        storage.clone(),
        transport.clone(),
        session.clone(),
        connection.clone(),
    );
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
            deliver_deferred(&storage, &transport, &session, &connection, call_id)
        }))
        .await;
    });
}

const MAX_RESULT_BYTES: usize = 1024 * 1024;
const MAX_STATE_BYTES: usize = 64 * 1024;
const MAX_EVENT_TEXT_BYTES: usize = 4 * 1024;
const MAX_EVENT_DATA_BYTES: usize = 64 * 1024;

fn json_len(value: &serde_json::Value) -> usize {
    serde_json::to_string(value)
        .map(|s| s.len())
        .unwrap_or(usize::MAX)
}

/// A `client_error` frame for this socket only.
pub(crate) fn client_error(
    code: &str,
    call_id: Option<&str>,
    message: impl Into<String>,
) -> serde_json::Value {
    let mut error = serde_json::json!({ "code": code, "message": message.into() });
    if let Some(call_id) = call_id {
        error["call_id"] = serde_json::json!(call_id);
    }
    serde_json::json!({ "client_error": error })
}

/// A refused answer to a question set (`specs/042-agent-asking-question` contracts/questions-wire.md
/// §2): the usual `client_error`, plus the set it concerns and — for an invalid answer — the first
/// question to blame and the rule it broke, so a client can put the message on that question and
/// keep what the person typed (FR-012).
pub(crate) fn question_error(
    set_id: &str,
    error: &vizier_core::questions::submit::SubmitError,
) -> serde_json::Value {
    let mut frame = client_error(error.code(), None, error.message());
    frame["client_error"]["set_id"] = serde_json::json!(set_id);
    if let Some(problem) = error.problems().first() {
        frame["client_error"]["rule"] = serde_json::json!(problem.rule);
        if let Some(question_id) = &problem.question_id {
            frame["client_error"]["question_id"] = serde_json::json!(question_id);
        }
    }
    frame
}

/// Whether `uid` is a client tool call in `session`: the scope of saved state and of a client
/// event's `source_call_id`, so a person cannot reach another's conversation by guessing ids.
async fn is_client_call_in(
    storage: &vizier_core::storage::VizierStorage,
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
pub(crate) fn client_tool_result(
    connection: &ClientConnection,
    payload: &serde_json::Value,
) -> Result<(), serde_json::Value> {
    let Some(call_id) = payload.get("call_id").and_then(|id| id.as_str()) else {
        return Err(client_error(
            "invalid_frame",
            None,
            "client_tool_result needs a call_id",
        ));
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
        return Err(client_error(
            "invalid_frame",
            Some(call_id),
            "client_tool_result needs ok or error",
        ));
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
pub(crate) async fn client_call_state(
    storage: &vizier_core::storage::VizierStorage,
    transport: &VizierTransport,
    session: &VizierSession,
    limits: &ClientLimits,
    payload: &serde_json::Value,
) -> Result<(), serde_json::Value> {
    let Some(call_id) = payload.get("call_id").and_then(|id| id.as_str()) else {
        return Err(client_error(
            "invalid_frame",
            None,
            "client_call_state needs a call_id",
        ));
    };
    let state = payload
        .get("state")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

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
            return Err(client_error(
                "state_not_saved",
                Some(call_id),
                "state not saved",
            ));
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
        return Err(client_error(
            "state_not_saved",
            Some(call_id),
            "state not saved",
        ));
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
pub(crate) async fn check_client_event(
    storage: &vizier_core::storage::VizierStorage,
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
    if event
        .data
        .as_ref()
        .is_some_and(|data| json_len(data) > MAX_EVENT_DATA_BYTES)
    {
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
        && !is_client_call_in(storage, session, source)
            .await
            .unwrap_or(false)
    {
        event.source_call_id = None;
    }
    Ok(())
}

/// Check what the client says about itself (`VizierRequest::client_context`): at most
/// `CLIENT_CONTEXT_MAX_ENTRIES` notes of `CLIENT_CONTEXT_MAX_BYTES` bytes each, after trimming.
/// Blank notes are dropped rather than refused — a client that sends an empty string has said
/// nothing, and the turn should not fail over it. Refused before the turn starts, so a client
/// learns the rule instead of silently losing its note.
///
/// The caps are the ones the context block renders with, so what reaches a prompt and what the
/// socket accepts cannot drift apart.
pub(crate) fn check_client_context(request: &mut VizierRequest) -> Result<(), serde_json::Value> {
    use vizier_core::agents::agent::system_prompt::context::{
        CLIENT_CONTEXT_MAX_BYTES, CLIENT_CONTEXT_MAX_ENTRIES,
    };

    let notes: Vec<String> = std::mem::take(&mut request.client_context)
        .into_iter()
        .map(|note| note.trim().to_string())
        .filter(|note| !note.is_empty())
        .collect();
    if notes.len() > CLIENT_CONTEXT_MAX_ENTRIES {
        return Err(client_error(
            "invalid_client_context",
            None,
            format!("at most {CLIENT_CONTEXT_MAX_ENTRIES} client context notes"),
        ));
    }
    if let Some(long) = notes.iter().find(|note| note.len() > CLIENT_CONTEXT_MAX_BYTES) {
        return Err(client_error(
            "invalid_client_context",
            None,
            format!(
                "a client context note is {} bytes; the limit is {CLIENT_CONTEXT_MAX_BYTES}",
                long.len()
            ),
        ));
    }
    request.client_context = notes;
    Ok(())
}

/// Move a request's checked declarations into its binding to `connection`. Only a request
/// that starts a chat turn takes client tools; on any other they are dropped unread.
pub(crate) fn bind_client_tools(
    request: &mut VizierRequest,
    connection: &std::sync::Arc<ClientConnection>,
    connections: &vizier_core::agents::client::ClientConnections,
) -> Result<(), serde_json::Value> {
    let tools = std::mem::take(&mut request.client_tools);
    let categories = std::mem::take(&mut request.client_categories);
    request.client = None;
    let takes_tools = matches!(
        request.content,
        VizierRequestContent::Chat(_)
            | VizierRequestContent::AudioChat(..)
            | VizierRequestContent::ClientEvent(_)
    );
    if tools.is_empty() || !takes_tools {
        return Ok(());
    }
    declare::validate(&tools, &categories).map_err(|err| err.frame())?;
    request.client = Some(connections.bind(connection, tools, categories));
    Ok(())
}

/// A reaction from this socket, applied through the shared ingest path. The new set reaches
/// every socket on the session (this one included, as its acknowledgement) through the
/// session-event broadcast; an `Err` is the message for a `reaction_error` to this socket only.
pub(crate) async fn webui_reaction(
    storage: &vizier_core::storage::VizierStorage,
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
            tracing::warn!(
                "failed to look up reaction target {}: {err}",
                payload.message_uid
            );
            return Err("reaction not saved".to_string());
        }
    }

    let username = curr_session
        .1
        .web_user()
        .unwrap_or("unknown")
        .to_string();
    let kind = match payload.action {
        ReactionAction::Added => ReactionKind::Add(payload.emoji.clone()),
        ReactionAction::Removed => ReactionKind::Remove(payload.emoji.clone()),
    };
    let change = ReactionChange {
        target: ReactionTarget::History(payload.message_uid.clone()),
        reactor: Reactor {
            id: username,
            name: None,
        },
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
    mut topic: crate::topic::TopicConnection,
    ws_idle_timeout_secs: u64,
    out_rx: flume::Receiver<serde_json::Value>,
) {
    let (mut writer, mut reader) = socket.split();
    let idle_timeout = Duration::from_secs(ws_idle_timeout_secs);

    let (write_tx, mut write_rx) = mpsc::channel::<Message>(64);
    let write_handle = tokio::spawn(async move {
        while let Some(msg) = write_rx.recv().await {
            if writer.send(msg).await.is_err() {
                break;
            }
        }
    });

    let outbound = topic.outbound().map(|o| tokio::spawn(o.run()));

    let mut ping_interval = tokio::time::interval(Duration::from_secs(30));
    let mut last_activity = tokio::time::Instant::now();
    let idle_deadline = tokio::time::sleep(idle_timeout);
    tokio::pin!(idle_deadline);

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                let _ = write_tx.send(Message::Ping(vec![].into())).await;
            }
            frame = out_rx.recv_async() => {
                match frame {
                    Ok(frame) => {
                        let _ = write_tx.send(Message::Text(frame.to_string().into())).await;
                    }
                    Err(_) => break,
                }
            }
            msg = reader.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<serde_json::Value>(&text) {
                            Ok(frame) => topic.handle_inbound(frame).await,
                            Err(_) => tracing::debug!("unrecognized WebSocket message: {}", &text[..text.len().min(200)]),
                        }
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Pong(_))) | Some(Ok(Message::Binary(_))) => {}
                    Some(Ok(Message::Ping(data))) => {
                        let _ = write_tx.send(Message::Pong(data)).await;
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
                tracing::warn!("websocket idle timeout");
                let _ = write_tx.send(Message::Close(None)).await;
                break;
            }
        }
    }

    topic.close();
    if let Some(handle) = outbound {
        handle.abort();
    }
    drop(write_tx);
    let _ = write_handle.await;
}

#[cfg(test)]
mod client_context_tests {
    use super::*;
    use vizier_core::agents::agent::system_prompt::context::{
        CLIENT_CONTEXT_MAX_BYTES, CLIENT_CONTEXT_MAX_ENTRIES,
    };

    fn request(notes: &[&str]) -> VizierRequest {
        VizierRequest {
            user: "alice".into(),
            content: VizierRequestContent::Chat("hi".into()),
            client_context: notes.iter().map(|note| note.to_string()).collect(),
            ..Default::default()
        }
    }

    /// A client's own words reach the turn trimmed, and the blanks it left behind are dropped
    /// rather than refused.
    #[test]
    fn a_well_formed_list_is_kept_trimmed_and_blank_notes_are_dropped() {
        let mut req = request(&["  This client renders mermaid.  ", "   ", ""]);
        assert!(check_client_context(&mut req).is_ok());
        assert_eq!(req.client_context, vec!["This client renders mermaid."]);
    }

    /// A request with no notes, or one that only had blanks, is an ordinary request.
    #[test]
    fn no_notes_stays_empty() {
        let mut req = request(&[]);
        assert!(check_client_context(&mut req).is_ok());
        assert!(req.client_context.is_empty());
    }

    /// Over the cap the note is refused before a turn starts, and the code names the rule so a
    /// client can fix it instead of guessing.
    #[test]
    fn too_many_notes_are_refused() {
        let many: Vec<String> = (0..CLIENT_CONTEXT_MAX_ENTRIES + 1).map(|i| format!("note {i}")).collect();
        let mut req = request(&many.iter().map(String::as_str).collect::<Vec<_>>());
        let frame = check_client_context(&mut req).unwrap_err();
        assert_eq!(frame["client_error"]["code"], "invalid_client_context");
        assert!(frame["client_error"]["message"].as_str().unwrap().contains("at most"));
    }

    /// A note over the byte cap is refused too: the block it becomes is bounded, so a client
    /// that wants to say more has to choose what matters.
    #[test]
    fn a_too_long_note_is_refused() {
        let long = "x".repeat(CLIENT_CONTEXT_MAX_BYTES + 1);
        let mut req = request(&[&long]);
        let frame = check_client_context(&mut req).unwrap_err();
        assert_eq!(frame["client_error"]["code"], "invalid_client_context");
        assert!(frame["client_error"]["message"].as_str().unwrap().contains("limit"));

        // At the cap it is fine, and the trimmed length is what counts.
        let at_cap = format!("{}  ", "x".repeat(CLIENT_CONTEXT_MAX_BYTES));
        let mut req = request(&[&at_cap]);
        assert!(check_client_context(&mut req).is_ok());
        assert_eq!(req.client_context[0].len(), CLIENT_CONTEXT_MAX_BYTES);
    }
}

#[cfg(test)]
mod question_error_tests {
    use super::*;
    use vizier_core::questions::{model::QuestionState, submit::SubmitError};

    /// An invalid answer names the set, the question to blame and the rule (contract §2), so the
    /// card can show it on that question and keep every field.
    #[test]
    fn an_invalid_answer_names_the_set_the_question_and_the_rule() {
        let frame = question_error(
            "set-1",
            &SubmitError::Invalid(vec![vizier_core::questions::validate::Problem::question_for_test(
                "db",
                "value_not_offered",
                "\"oracle\" is not one of this question's options",
            )]),
        );
        let error = &frame["client_error"];
        assert_eq!(error["code"], "invalid_answer");
        assert_eq!(error["set_id"], "set-1");
        assert_eq!(error["question_id"], "db");
        assert_eq!(error["rule"], "value_not_offered");
    }

    /// A closed set names the set and nothing else to blame.
    #[test]
    fn a_closed_set_names_only_the_set() {
        let frame = question_error("set-1", &SubmitError::AlreadyClosed(QuestionState::Answered));
        let error = &frame["client_error"];
        assert_eq!(error["code"], "question_not_open");
        assert_eq!(error["set_id"], "set-1");
        assert!(error.get("question_id").is_none() && error.get("rule").is_none());
    }
}
