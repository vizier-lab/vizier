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

use super::{platform_of_slug, user_can_show_platform_chats, user_can_view_agent};
use vizier_core::access::Caller;
use vizier_core::utils::rate_limit::RateLimiter;
use vizier_core::artifacts::access as ArtifactsAccess;
use crate::models::page::Page;
use vizier_core::schema::session_file::SessionFileRecord;
use vizier_core::storage::session_file::SessionFileStorage;

/// The channel a read-only route means by `channel_id`: a Discord or Telegram conversation by its
/// slug (`discord__123`, `telegram__-9`, which the conversation picker lists), anything else a
/// WebUI channel of the caller's own. Routes that write never use this: nobody chats on Discord
/// through the WebUI.
fn readable_channel(username: &str, channel_id: &str) -> (VizierChannelId, bool) {
    if let Some(id) = channel_id
        .strip_prefix("discord__")
        .and_then(|id| id.parse::<u64>().ok())
    {
        return (VizierChannelId::DiscordChanel(id), true);
    }
    if let Some(id) = channel_id
        .strip_prefix("telegram__")
        .and_then(|id| id.parse::<i64>().ok())
    {
        return (VizierChannelId::TelegramChannel(id), true);
    }
    // A blocking subagent's conversation is addressable by its topic, even though it has
    // no background job row of its own.
    if channel_id.eq_ignore_ascii_case("SUBAGENT") {
        return (VizierChannelId::Subagent, false);
    }
    (
        VizierChannelId::HTTP(username.to_string(), channel_id.to_string()),
        false,
    )
}

/// The session a read-only route opens. Discord and Telegram conversations have no topic.
fn readable_session(
    agent_id: String,
    username: &str,
    channel_id: &str,
    topic_id: TopicId,
) -> VizierSession {
    let (channel, platform) = readable_channel(username, channel_id);
    VizierSession(
        agent_id,
        channel,
        if platform { None } else { Some(topic_id) },
    )
}

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
) -> models::response::Response<Vec<SessionHistory>> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
        }
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    if let Some(platform) = platform_of_slug(&channel_id) {
        if !user_can_show_platform_chats(&user, &config, platform) {
            return err_response(StatusCode::FORBIDDEN, "Access denied".into());
        }
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
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<Page<SessionFileRecord>> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
        }
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    if let Some(platform) = platform_of_slug(&channel_id) {
        if !user_can_show_platform_chats(&user, &config, platform) {
            return err_response(StatusCode::FORBIDDEN, "Access denied".into());
        }
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
    api_response(
        StatusCode::OK,
        Page {
            items,
            total,
            counts: Default::default(),
        },
    )
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
) -> models::response::Response<vizier_core::session_files::ClearResult> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
        }
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }
    if let Some(platform) = platform_of_slug(&channel_id) {
        if !user_can_show_platform_chats(&user, &config, platform) {
            return err_response(StatusCode::FORBIDDEN, "Access denied".into());
        }
    }
    let session = readable_session(agent_id, &user.username, &channel_id, topic_id);
    match state.deps.session_files.clear(&session).await {
        Ok(result) => api_response(StatusCode::OK, result),
        Err(vizier_core::session_files::ClearError::TurnActive) => err_response(
            StatusCode::CONFLICT,
            "a turn is running or queued in this conversation; wait for it to finish".into(),
        ),
        Err(vizier_core::session_files::ClearError::AgentNotFound) => {
            err_response(StatusCode::NOT_FOUND, "agent not found".into())
        }
        Err(vizier_core::session_files::ClearError::Storage(e)) => {
            err_response(StatusCode::INTERNAL_SERVER_ERROR, e)
        }
    }
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
) -> models::response::Response<Artifact> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
        }
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }
    if let Some(platform) = platform_of_slug(&channel_id) {
        if !user_can_show_platform_chats(&user, &config, platform) {
            return err_response(StatusCode::FORBIDDEN, "Access denied".into());
        }
    }
    if !ArtifactsAccess::can_manage(&Caller::from(&user), &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let session = readable_session(agent_id.clone(), &user.username, &channel_id, topic_id);
    let source = vizier_core::artifacts::SourceFile {
        session,
        filename: body.filename.clone(),
    };
    match state
        .deps
        .artifacts
        .promote(
            &agent_id,
            Requester::User(user.user_id.clone()),
            &source,
            body.name.as_deref(),
        )
        .await
    {
        Ok(artifact) => api_response(StatusCode::CREATED, artifact),
        Err(e) => {
            let message = e.to_string();
            let status = if message.contains("already exists") {
                StatusCode::CONFLICT
            } else if message.contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            };
            err_response(status, message)
        }
    }
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
) -> models::response::Response<Vec<TopicEntry>> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
        }
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
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<TopicEntry> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
        }
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    if let Some(platform) = platform_of_slug(&channel_id) {
        if !user_can_show_platform_chats(&user, &config, platform) {
            return err_response(StatusCode::FORBIDDEN, "Access denied".into());
        }
    }

    let (channel, platform) = readable_channel(&user.username, &channel_id);

    match state
        .storage
        .get_session_detail_by_topic(
            agent_id,
            channel,
            if platform { None } else { Some(topic_id) },
        )
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
    Extension(user): Extension<crate::auth::Principal>,
) -> models::response::Response<String> {
    let config = match state.storage.get_agent(&agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
        }
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !user_can_view_agent(&user, &config) {
        return err_response(StatusCode::FORBIDDEN, "Access denied".into());
    }

    let session = VizierSession(
        agent_id,
        VizierChannelId::HTTP(user.username, channel_id),
        Some(topic_id),
    );
    if state.deps.session_files.has_active_turn(&session).await {
        return err_response(
            StatusCode::CONFLICT,
            "a turn is running or queued in this conversation; wait for it to finish".into(),
        );
    }
    match state.deps.session_files.delete_conversation(&session).await {
        Ok(_) => api_response(StatusCode::OK, "Topic deleted".into()),
        Err(vizier_core::session_files::ClearError::TurnActive) => err_response(
            StatusCode::CONFLICT,
            "a turn is running or queued in this conversation; wait for it to finish".into(),
        ),
        Err(vizier_core::session_files::ClearError::AgentNotFound) => {
            err_response(StatusCode::NOT_FOUND, "Topic not found".into())
        }
        Err(vizier_core::session_files::ClearError::Storage(e)) => {
            err_response(StatusCode::INTERNAL_SERVER_ERROR, e)
        }
    }
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
