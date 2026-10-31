use axum::{
    Extension, Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    routing::{any, delete, get, post},
};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::mpsc;

use vizier_core::{
    agents::client::{
        ClientAnswer, ClientBinding, ClientConnection, DeferredClaims, declare, deliver_deferred,
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
    pub before: Option<chrono::DateTime<Utc>>,
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
    /// Deferred calls being delivered by some socket right now.
    pub deferred: std::sync::Arc<DeferredClaims>,
}

/// Deliver deferred calls to this socket in the background (research R18), so a slow answer
/// never holds up the socket's own loop.
pub(crate) fn spawn_delivery(
    storage: &std::sync::Arc<vizier_core::storage::VizierStorage>,
    transport: &VizierTransport,
    session: &VizierSession,
    connection: &std::sync::Arc<ClientConnection>,
    claims: &std::sync::Arc<DeferredClaims>,
    call_ids: Option<Vec<String>>,
) {
    let (storage, transport, session, connection, claims) = (
        storage.clone(),
        transport.clone(),
        session.clone(),
        connection.clone(),
        claims.clone(),
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
            deliver_deferred(
                &storage,
                &transport,
                &session,
                &connection,
                &claims,
                call_id,
            )
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

/// Move a request's checked declarations into its binding to `connection`. Only a request
/// that starts a chat turn takes client tools; on any other they are dropped unread.
pub(crate) fn bind_client_tools(
    request: &mut VizierRequest,
    connection: &std::sync::Arc<ClientConnection>,
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
