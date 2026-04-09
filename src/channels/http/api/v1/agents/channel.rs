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
            response::{api_response, err_response},
        },
        state::HTTPState,
    },
    schema::{
        SessionHistory, TopicId, VizierChannelId, VizierRequest, VizierSessionDetail, VizierSession,
    },
    storage::{history::HistoryStorage, session::SessionStorage},
    transport::VizierTransport,
};

pub fn channel() -> Router<HTTPState> {
    Router::new()
        .route("/{channel_id}/topics", get(list_topics))
        .route("/{channel_id}/topic/{topic_id}/chat", any(chat))
        .route("/{channel_id}/topic/{topic_id}/history", get(get_topic_history))
        .route("/{channel_id}/topic/{topic_id}", delete(delete_topic))
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    before: Option<chrono::DateTime<Utc>>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, serde::Serialize)]
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

pub async fn get_topic_history(
    Path((agent_id, channel_id, topic_id)): Path<(String, String, TopicId)>,
    Query(params): Query<HistoryQuery>,
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<SessionHistory>> {
    if !state.config.is_agent_exists(&agent_id) {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    let session = VizierSession(
        agent_id,
        VizierChannelId::HTTP(channel_id),
        Some(topic_id),
    );

    let response = state
        .storage
        .list_session_history(session, params.before, params.limit)
        .await;

    if response.is_err() {
        return err_response(StatusCode::NOT_FOUND, "Not found".into());
    }

    api_response(StatusCode::OK, response.unwrap())
}

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
        .get_session_list(agent_id, channel)
        .await;

    if response.is_err() {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch topics".into());
    }

    let list = response
        .unwrap()
        .into_iter()
        .map(TopicEntry::from)
        .collect();

    api_response(StatusCode::OK, list)
}

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
    let session = VizierSession(
        agent_id,
        VizierChannelId::HTTP(channel_id),
        Some(topic_id),
    );

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
                        let _ = transport
                            .send_request(curr_session.clone(), request)
                            .await;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    }

    handle.abort();
}
