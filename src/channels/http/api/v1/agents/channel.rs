use axum::{
    Router,
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

use crate::{
    channels::http::{
        models::{
            self,
            response::{APIResponse, api_response, err_response},
        },
        state::HTTPState,
    },
    schema::{
        SessionHistory, TopicId, VizierAttachmentContent, VizierChannelId, VizierRequest,
        VizierSession, VizierSessionDetail,
    },
    storage::{history::HistoryStorage, session::SessionStorage},
    transport::VizierTransport,
};

pub fn channel() -> Router<HTTPState> {
    Router::new()
        .route("/{channel_id}/topics", get(list_topics))
        .route("/{channel_id}/topic/{topic_id}/chat", any(chat))
        .route(
            "/{channel_id}/topic/{topic_id}/history",
            get(get_topic_history),
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
}

impl From<VizierSessionDetail> for TopicEntry {
    fn from(detail: VizierSessionDetail) -> Self {
        Self {
            topic_id: detail.topic.unwrap_or_default(),
            title: detail.title,
            agent_id: detail.agent_id,
            channel: format!("{:?}", detail.channel),
        }
    }
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
) -> models::response::Response<Vec<SessionHistory>> {
    if !state.config.is_agent_exists(&agent_id) {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    let session = VizierSession(agent_id, VizierChannelId::HTTP(channel_id), Some(topic_id));

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
) -> models::response::Response<Vec<TopicEntry>> {
    if !state.config.is_agent_exists(&agent_id) {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    let channel = VizierChannelId::HTTP(channel_id);

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
) -> models::response::Response<String> {
    if !state.config.is_agent_exists(&agent_id) {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    let channel = VizierChannelId::HTTP(channel_id);

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
) -> axum::response::Response {
    if !state.config.is_agent_exists(&agent_id) {
        return axum::response::Response::builder()
            .status(404)
            .body(axum::body::Body::empty())
            .unwrap();
    }

    let transport = state.transport.clone();
    let session = VizierSession(agent_id, VizierChannelId::HTTP(channel_id), Some(topic_id));

    ws.on_upgrade(move |socket| handle_socket(socket, session, transport))
}

pub async fn handle_socket(
    socket: WebSocket,
    curr_session: VizierSession,
    transport: VizierTransport,
) {
    let (mut writer, mut reader) = socket.split();

    let mut recv = transport.subscribe_response().await.unwrap();
    let req_session = curr_session.clone();
    let handle = tokio::spawn(async move {
        while let Ok((session, response)) = recv.recv().await {
            if session != req_session {
                continue;
            }
            let _ = writer
                .send(axum::extract::ws::Message::Text(
                    serde_json::to_string(&response).unwrap().into(),
                ))
                .await;
        }
    });

    loop {
        if let Some(Ok(message)) = reader.next().await {
            match message {
                Message::Text(text) => {
                    if let Ok(request) = serde_json::from_str::<VizierRequest>(&text.to_string()) {
                        let mut request = request.clone();
                        for attachment in request.attachments.iter_mut() {
                            if let VizierAttachmentContent::Url(url) = &attachment.content {
                                if let Ok(response) = reqwest::get(url).await {
                                    if response.status().is_success() {
                                        if let Ok(bytes) = response.bytes().await {
                                            attachment.content =
                                                VizierAttachmentContent::Bytes(bytes.to_vec());
                                        }
                                    }
                                }
                            }
                        }

                        let _ = transport.send_request(curr_session.clone(), request).await;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    }

    handle.abort();
}
