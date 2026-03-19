use axum::{
    Router,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    routing::{any, get},
};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use reqwest::StatusCode;

use crate::{
    channels::http::{
        models::{
            self,
            response::{api_response, err_response},
            session::{ChatHistory, ChatRequest, ChatResponse},
        },
        state::HTTPState,
    },
    schema::{SessionHistoryContent, SessionId, VizierRequest, VizierSession},
    transport::VizierTransport,
};

pub fn session() -> Router<HTTPState> {
    Router::new()
        .route("/{session_id}/chat", any(chat))
        .route("/{session_id}/history", get(get_session_history))
}

pub async fn get_session_history(
    Path((agent_id, session_id)): Path<(String, String)>,
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<ChatHistory>> {
    if !state.config.is_agent_exists(&agent_id) {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    let response = state
        .db
        .list_session_history(VizierSession(agent_id, SessionId::HTTP(session_id)))
        .await;

    if response.is_err() {
        return err_response(StatusCode::NOT_FOUND, "Not found".into());
    }

    let list = response
        .unwrap()
        .iter()
        .map(|history| match history.content.clone() {
            SessionHistoryContent::Request(req) => ChatHistory::request(ChatRequest {
                content: req.content,
                user: req.user,
                timestamp: Some(history.timestamp),
            }),
            SessionHistoryContent::Response(content, _) => ChatHistory::response(ChatResponse {
                content: Some(content),
                choice: None,
                thinking: false,
                timestamp: Some(history.timestamp),
            }),
        })
        .collect();

    api_response(StatusCode::OK, list)
}

pub async fn chat(
    Path((agent_id, session_id)): Path<(String, String)>,
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
    ws.on_upgrade(|socket| {
        handle_socket(
            socket,
            VizierSession(agent_id, SessionId::HTTP(session_id)),
            transport,
        )
    })
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
                    serde_json::to_string(&ChatResponse::from(response))
                        .unwrap()
                        .into(),
                ))
                .await;
        }
    });

    loop {
        if let Some(Ok(message)) = reader.next().await {
            match message {
                Message::Text(text) => {
                    if let Ok(request) = serde_json::from_str::<ChatRequest>(&text.to_string()) {
                        let metadata = serde_json::json!({
                            "sent_at": Utc::now().to_string(),
                            "websocket_session_id": curr_session.1.clone(),
                        });

                        let _ = transport
                            .send_request(
                                curr_session.clone(),
                                VizierRequest {
                                    user: request.user,
                                    content: request.content,
                                    is_silent_read: false,
                                    metadata,
                                    ..Default::default()
                                },
                            )
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
