use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use chrono::Utc;
use flume::{Receiver, Sender};
use futures::future::join_all;
use serde_json::json;
use tokio::sync::Mutex;

use crate::{
    channels::http::models::session::{ChatRequest, ChatResponse},
    config::VizierConfig,
    database::VizierDatabases,
    schema::{AgentId, SessionId, VizierRequest, VizierResponse, VizierSession},
    transport::{VizierTransport, VizierTransportChannel},
};

#[derive(Debug, Clone)]
pub struct HTTPState {
    pub config: Arc<VizierConfig>,
    pub transport: ChatTransport,
    pub db: VizierDatabases,
}

pub type ChatRequestTransport = (
    Sender<((AgentId, String), ChatRequest)>,
    Receiver<((AgentId, String), ChatRequest)>,
);

pub type ChatReponseTransport = (Sender<ChatResponse>, Receiver<ChatResponse>);

#[derive(Debug, Clone)]
pub struct ChatTransport {
    pub requests: ChatRequestTransport,
    pub reponses: Arc<Mutex<HashMap<(AgentId, String), ChatReponseTransport>>>,
}

impl ChatTransport {
    pub fn new() -> Self {
        Self {
            requests: flume::unbounded(),
            reponses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run(&self, transport: VizierTransport) -> Result<()> {
        let req_transport = transport.clone();
        let req_chat_transport = self.clone();
        let request_handle = tokio::spawn(async move {
            while let Ok(((agent_id, session_id), request)) =
                req_chat_transport.requests.1.recv_async().await
            {
                let metadata = json!({
                    "sent_at": Utc::now().to_string(),
                    "websocket_session_id": session_id,
                });

                let _ = req_transport
                    .send_request(
                        VizierSession(agent_id, SessionId::HTTP(session_id)),
                        VizierRequest {
                            user: request.user,
                            content: request.content,
                            metadata,
                            ..Default::default()
                        },
                    )
                    .await;
            }
        });

        let res_transport = transport.clone();
        let res_chat_transport = self.clone();
        let response_handle = tokio::spawn(async move {
            while let Ok((VizierSession(agent_id, SessionId::HTTP(session_id)), response)) =
                res_transport
                    .read_response(VizierTransportChannel::HTTP)
                    .await
            {
                let response_transport = res_chat_transport.reponses.lock().await;
                if let Some(transport) = response_transport.get(&(agent_id, session_id)).clone() {
                    let (writer, _) = transport.clone();
                    match response {
                        VizierResponse::Thinking => {
                            let _ = writer
                                .send_async(ChatResponse {
                                    thinking: true,
                                    ..Default::default()
                                })
                                .await;
                        }
                        VizierResponse::Message(content) => {
                            let _ = writer
                                .send_async(ChatResponse {
                                    content,
                                    thinking: false,
                                    ..Default::default()
                                })
                                .await;
                        }
                    }
                }
            }
        });

        for res in join_all(vec![request_handle, response_handle]).await {
            res?;
        }

        Ok(())
    }
}
