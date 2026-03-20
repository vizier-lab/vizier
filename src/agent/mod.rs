use anyhow::Result;
use chrono::Utc;
use flume::{Receiver, Sender};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, MutexGuard};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::agent::agent_impl::VizierAgent;
use crate::agent::memory::SessionMemories;
use crate::config::agent::AgentConfig;
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::{
    SessionHistoryContent, VizierRequest, VizierResponse, VizierResponseStats, VizierSession,
};
use crate::utils::remove_think_tags;

pub mod agent_impl;
pub mod exec;
pub mod hook;
pub mod memory;
pub mod tools;

#[derive(Clone)]
pub struct VizierAgents {
    deps: VizierDependencies,
}

impl VizierAgent {
    async fn handle_silent_read(
        &self,
        mut session: MutexGuard<'_, AgentSession>,
        request: &VizierRequest,
    ) -> Result<()> {
        self.silent_read(request.clone(), &session.session_memory)
            .await?;

        session.session_memory.push_user_message(request.clone());
        session.session_memory.try_summarize(self).await?;

        session.last_interact_at = Utc::now();

        Ok(())
    }

    async fn handle_chat(
        &self,
        request: &VizierRequest,
        mut session: MutexGuard<'_, AgentSession>,
    ) -> Result<String> {
        let response = self.chat(request.clone(), &session.session_memory).await?;

        let response_msg = remove_think_tags(&*response);

        session.session_memory.push_user_message(request.clone());
        session.session_memory.push_agent(response_msg);
        session.session_memory.try_summarize(&self).await?;

        session.last_interact_at = Utc::now();
        Ok(response.to_string())
    }
}

type SessionTransport = (Sender<VizierRequest>, Receiver<VizierRequest>);

impl VizierAgents {
    pub async fn new(deps: VizierDependencies) -> Result<Self> {
        Ok(Self { deps })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut sessions: HashMap<VizierSession, SessionProcess> = HashMap::new();

        let transport = self.deps.transport.clone();
        let mut recv = transport.subscribe_request().await?;

        while let Ok((session, request)) = recv.recv().await {
            if let Some(process) = sessions.get(&session) {
                if !process.handle.is_finished() {
                    let _ = process.session_transport.0.send_async(request).await;
                    continue;
                }
            }

            let agent_config = self
                .deps
                .config
                .agents
                .get(&session.0)
                .ok_or(VizierError("agent not found".into()))?;

            let process =
                SessionProcess::new(agent_config.clone(), session.clone(), self.deps.clone())
                    .await?;

            let _ = process.session_transport.0.send_async(request).await;

            sessions.insert(session, process);
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct AgentSession {
    session_memory: SessionMemories,
    session_ttl: Duration,
    last_interact_at: chrono::DateTime<Utc>,
}

impl AgentSession {
    fn lobotomy(&mut self) {
        self.last_interact_at = Utc::now();
        self.session_memory.flush();
    }

    fn is_stale(&self) -> bool {
        let diff = Utc::now() - self.last_interact_at;

        diff.to_std().unwrap() > self.session_ttl
    }
}

struct SessionProcess {
    session_transport: Arc<SessionTransport>,
    handle: JoinHandle<()>,
}

impl SessionProcess {
    async fn new(
        agent_config: AgentConfig,
        session: VizierSession,
        deps: VizierDependencies,
    ) -> Result<Self> {
        let agent = Arc::new(VizierAgent::new(&deps, session.clone()).await?);

        let session_transport = Arc::new(flume::unbounded());

        let response_session = session.clone();
        let send_response = async move |response| {
            let res = deps
                .transport
                .send_response(response_session, response)
                .await;

            res
        };

        let request_session = session.clone();
        let request_database = deps.database.clone();
        let save_request = async move |request: VizierRequest| {
            let res = request_database
                .save_session_history(
                    request_session.clone(),
                    SessionHistoryContent::Request(request.clone()),
                )
                .await;

            res
        };

        let response_session = session.clone();
        let response_database = deps.database.clone();
        let save_response = async move |response: String| {
            let res = response_database
                .save_session_history(
                    response_session.clone(),
                    SessionHistoryContent::Response(response, None),
                )
                .await;

            res
        };

        Ok(Self {
            session_transport: session_transport.clone(),
            handle: tokio::spawn(async move {
                let agent_config = agent_config.clone();
                let session = Arc::new(Mutex::new(AgentSession {
                    session_memory: SessionMemories::new(agent_config.session_memory.clone()),
                    session_ttl: *agent_config.session_ttl,
                    last_interact_at: Utc::now(),
                }));

                let main_session = session.clone();
                let main_handler = tokio::spawn(async move {
                    let send_response = send_response.clone();
                    while let Ok(request) = session_transport.1.recv_async().await {
                        let _ = save_request(request.clone()).await;

                        let start = Instant::now();

                        let mut main_session = main_session.lock().await;
                        let send_lobotomy = send_response.clone();
                        if request.content == "/lobotomy" {
                            let _ = main_session.lobotomy();
                            let _ = save_response("YIPEEEE".into()).await;

                            tokio::spawn(async move {
                                if let Err(err) = send_lobotomy(VizierResponse::Message {
                                    content: "YIPEEEE".into(),
                                    stats: None,
                                })
                                .await
                                {
                                    log::error!("{}", err);
                                }
                            });

                            continue;
                        }

                        if request.is_silent_read {
                            let _ = agent.handle_silent_read(main_session, &request).await;
                            continue;
                        }

                        let send_thinking = send_response.clone();
                        let thinking = tokio::spawn(async move {
                            loop {
                                let _ =
                                    send_thinking.clone()(VizierResponse::ThinkingProgress).await;

                                tokio::time::sleep(Duration::from_secs(5)).await;
                            }
                        });

                        let content = agent.handle_chat(&request, main_session).await;
                        let send_response = send_response.clone();
                        match content {
                            Err(err) => {
                                let _ = save_response(err.to_string()).await;
                                if let Err(err) = send_response(VizierResponse::Message {
                                    content: err.to_string(),
                                    stats: None,
                                })
                                .await
                                {
                                    log::error!("{}", err);
                                }
                            }
                            Ok(content) => {
                                let _ = save_response(content.clone()).await;
                                if let Err(err) = send_response(VizierResponse::Message {
                                    content,
                                    stats: Some(VizierResponseStats {
                                        duration: start.elapsed(),
                                    }),
                                })
                                .await
                                {
                                    log::error!("{}", err);
                                }
                            }
                        }

                        thinking.abort();
                    }
                });

                let session = session.clone();
                let session_ttl = agent_config.session_ttl;
                let stale_handler = tokio::spawn(async move {
                    loop {
                        let _ = tokio::time::sleep(*session_ttl).await;
                        let session = session.lock().await;
                        if session.is_stale() {
                            log::info!("{:?} session stale", session.clone());
                            main_handler.abort();
                            return;
                        }
                    }
                });

                let _ = stale_handler.await;
            }),
        })
    }
}
