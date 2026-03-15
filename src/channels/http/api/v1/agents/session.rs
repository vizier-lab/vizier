use axum::{
    Router,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    routing::{any, delete, get, post},
};
use futures::{SinkExt, StreamExt};
use reqwest::StatusCode;

use crate::channels::http::{
    models::{
        self,
        response::{api_response, err_response},
        session::{ChatRequest, SessionResponse},
    },
    state::{ChatReponseTransport, ChatRequestTransport, HTTPState},
};

pub fn session() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list_sessions))
        .route("/", post(create_session))
        .route("/{session_id}", post(create_custom_session))
        .route("/{session_id}", delete(delete_sessions))
        .route("/{session_id}/chat", any(chat))
}

pub async fn create_custom_session(
    Path((agent_id, session_id)): Path<(String, String)>,
    State(state): State<HTTPState>,
) -> models::response::Response<SessionResponse> {
    if !state.config.is_agent_exists(&agent_id) {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    let mut sessions = state.transport.reponses.lock().await;
    // skip if already exists
    if sessions
        .get_mut(&(agent_id.clone(), session_id.clone()))
        .is_some()
    {
        return api_response(
            StatusCode::OK,
            SessionResponse {
                agent_id,
                session_id,
            },
        );
    }

    let session = (agent_id.clone(), session_id.clone());
    sessions.insert(session, flume::unbounded());

    api_response(
        StatusCode::OK,
        SessionResponse {
            agent_id,
            session_id,
        },
    )
}

pub async fn delete_sessions(
    Path((agent_id, session_id)): Path<(String, String)>,
    State(state): State<HTTPState>,
) -> models::response::Response<()> {
    if !state.config.is_agent_exists(&agent_id) {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    let _ = state
        .transport
        .reponses
        .lock()
        .await
        .remove(&(agent_id, session_id));

    api_response(StatusCode::OK, ())
}

pub async fn list_sessions(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<SessionResponse>> {
    if !state.config.is_agent_exists(&agent_id) {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    let sessions = state
        .transport
        .reponses
        .lock()
        .await
        .iter()
        .map(|((agent_id, session_id), _)| SessionResponse {
            agent_id: agent_id.clone(),
            session_id: session_id.clone(),
        })
        .collect::<Vec<_>>();

    api_response(StatusCode::OK, sessions)
}

pub async fn create_session(
    Path(agent_id): Path<String>,
    State(state): State<HTTPState>,
) -> models::response::Response<SessionResponse> {
    if !state.config.is_agent_exists(&agent_id) {
        return err_response(StatusCode::NOT_FOUND, format!("agent {agent_id} not found"));
    }

    let session_id = nanoid::nanoid!(10);
    state
        .transport
        .reponses
        .lock()
        .await
        .insert((agent_id.clone(), session_id.clone()), flume::unbounded());

    api_response(
        StatusCode::OK,
        SessionResponse {
            agent_id,
            session_id,
        },
    )
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

    let mut responses = state.transport.reponses.lock().await;
    let session = responses
        .entry((agent_id.clone(), session_id.clone()))
        .or_insert(flume::unbounded());

    let requests = state.transport.requests.clone();
    let responses = session.clone();
    ws.on_upgrade(|socket| handle_socket(socket, agent_id, session_id, requests, responses))
}

pub async fn handle_socket(
    socket: WebSocket,
    agent_id: String,
    session_id: String,
    requests: ChatRequestTransport,
    responses: ChatReponseTransport,
) {
    let (mut writer, mut reader) = socket.split();
    let handle = tokio::spawn(async move {
        loop {
            if let Ok(response) = responses.1.recv_async().await {
                let _ = writer
                    .send(axum::extract::ws::Message::Text(
                        serde_json::to_string(&response).unwrap().into(),
                    ))
                    .await;
            }
        }
    });

    loop {
        if let Some(Ok(message)) = reader.next().await {
            match message {
                Message::Text(text) => {
                    if let Ok(request) = serde_json::from_str::<ChatRequest>(&text.to_string()) {
                        log::debug!("{:?}", request);
                        let _ = requests
                            .0
                            .send_async(((agent_id.clone(), session_id.clone()), request))
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
