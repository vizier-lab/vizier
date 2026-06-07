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
            state::HTTPState,
        },
        reaction_store,
    },
    schema::{
        PlatformMessageId, ReactionAction, ReactionEntry, ReactionEvent, SessionHistory, TopicId,
        VizierAttachmentContent, VizierChannelId, VizierRequest, VizierRequestContent,
        VizierSession, VizierSessionDetail,
    },
    storage::{agent::AgentStorage, history::HistoryStorage, session::SessionStorage},
    transport::VizierTransport,
};

use super::user_can_view_agent;

pub fn channel() -> Router<HTTPState> {
    Router::new()
        .route("/{channel_id}/topics", get(list_topics))
        .route("/{channel_id}/topic/{topic_id}/chat", any(chat))
        .route(
            "/{channel_id}/topic/{topic_id}/history",
            get(get_topic_history),
        )
        .route(
            "/{channel_id}/topic/{topic_id}/detail",
            get(get_topic_detail),
        )
        .route("/{channel_id}/topic/{topic_id}", delete(delete_topic))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct HistoryQuery {
    before: Option<chrono::DateTime<Utc>>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct TopicEntry {
    pub topic_id: String,
    pub title: String,
    pub agent_id: String,
    pub channel: String,
    pub is_thinking: bool,
}

impl From<VizierSessionDetail> for TopicEntry {
    fn from(detail: VizierSessionDetail) -> Self {
        Self {
            topic_id: detail.topic.unwrap_or_default(),
            title: detail.title,
            agent_id: detail.agent_id,
            channel: format!("{:?}", detail.channel),
            is_thinking: detail.is_thinking,
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

    let session = VizierSession(agent_id, VizierChannelId::HTTP(user.username, channel_id), Some(topic_id));

    let response = state
        .storage
        .list_session_history(session, params.before, params.limit)
        .await;

    if response.is_err() {
        return err_response(StatusCode::NOT_FOUND, "Not found".into());
    }

    api_response(StatusCode::OK, response.unwrap())
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
        .get_session_list(agent_id, Some(channel))
        .await;

    if response.is_err() {
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch topics".into(),
        );
    }

    let list = response
        .unwrap()
        .into_iter()
        .map(TopicEntry::from)
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

    let channel = VizierChannelId::HTTP(user.username, channel_id);

    match state
        .storage
        .get_session_detail_by_topic(agent_id, channel, Some(topic_id))
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

    ws.on_upgrade(move |socket| handle_socket(socket, session, transport, state.storage.clone(), state.config.workspace.clone(), ws_idle_timeout_secs))
}

pub async fn handle_socket(
    socket: WebSocket,
    curr_session: VizierSession,
    transport: VizierTransport,
    storage: std::sync::Arc<crate::storage::VizierStorage>,
    workspace: String,
    ws_idle_timeout_secs: u64,
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

    let mut ping_interval = tokio::time::interval(Duration::from_secs(30));
    let mut last_activity = tokio::time::Instant::now();
    let mut idle_deadline = tokio::time::sleep(idle_timeout);
    tokio::pin!(idle_deadline);

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                let _ = write_tx.send(Message::Ping(vec![].into())).await;
            }
            msg = reader.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text_str = text.to_string();
                        
                        if let Ok(reaction_msg) = serde_json::from_str::<WebSocketReactionMessage>(&text_str) {
                            tracing::info!("received reaction from WebUI: {:?}", reaction_msg);
                            let payload = reaction_msg.reaction;
                            let username = match &curr_session.1 {
                                VizierChannelId::HTTP(user, _) => user.clone(),
                                _ => "unknown".to_string(),
                            };
                            let entry = ReactionEntry {
                                user_id: username.clone(),
                                emoji: payload.emoji.clone(),
                            };
                            
                            if let Err(e) = reaction_store::record_reaction(&storage, &curr_session, &payload.message_uid, entry).await {
                                tracing::error!("failed to record reaction: {:?}", e);
                            } else {
                                tracing::info!("reaction recorded for message_uid: {}", payload.message_uid);
                            }

                            let event = ReactionEvent {
                                platform_message_id: None,
                                user_id: username,
                                emoji: payload.emoji,
                                action: payload.action,
                            };

                            let session = curr_session.clone();
                            let _ = transport.send_request(
                                session,
                                VizierRequest {
                                    timestamp: Utc::now(),
                                    user: "system".to_string(),
                                    content: VizierRequestContent::Reaction(event),
                                    metadata: serde_json::json!({}),
                                    attachments: vec![],
                                    ..Default::default()
                                },
                                None,
                            ).await;
                        } else if let Ok(request) = serde_json::from_str::<VizierRequest>(&text_str) {
                            let mut request = request.clone();
                            for attachment in request.attachments.iter_mut() {
                                if let VizierAttachmentContent::Url(_) = &attachment.content {
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
                            }

                            let (response_tx, response_rx) = flume::unbounded();
                            let session = curr_session.clone();
                            if let Err(err) = transport.send_request(session.clone(), request, Some(response_tx)).await {
                                tracing::error!("failed to send request: {}", err);
                                continue;
                            }

                            let resp_tx = write_tx.clone();
                            tokio::spawn(async move {
                                while let Ok(response) = response_rx.recv_async().await {
                                    if let Ok(json) = serde_json::to_string(&response) {
                                        if resp_tx.send(Message::Text(json.into())).await.is_err() {
                                            break;
                                        }
                                    }
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

    drop(write_tx);
    let _ = write_handle.await;
}
