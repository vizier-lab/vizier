use anyhow::Result;
use chrono::Utc;
use flume::{Receiver, Sender};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, MutexGuard};
use tokio::task::JoinHandle;

use crate::agent::agent_impl::VizierAgent;
use crate::agent::memory::SessionMemories;
use crate::agent::session::VizierSession;
use crate::config::agent::AgentConfig;
use crate::dependencies::VizierDependencies;
use crate::transport::{VizierRequest, VizierResponse, VizierTransport};
use crate::utils::remove_think_tags;

pub mod agent_impl;
pub mod exec;
pub mod memory;
pub mod session;
pub mod tools;

#[derive(Clone)]
pub struct VizierAgents {
    deps: VizierDependencies,
    agents: HashMap<String, Arc<VizierAgent>>,
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
        let mut agents = HashMap::new();

        let config = deps.config.clone();
        for (agent_id, _) in config.agents.iter() {
            agents.insert(
                agent_id.clone(),
                Arc::new(VizierAgent::new(&mut deps.clone(), agent_id.clone())?),
            );
        }

        Ok(Self {
            deps,
            agents: agents,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut sessions: HashMap<VizierSession, SessionProcess> = HashMap::new();

        let transport = self.deps.transport.clone();

        while let Ok((session, request)) = transport.read_request().await {
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
                .get(&session.0.clone())
                .ok_or(anyhow::Error::msg("Agent not found"))?;

            let agent = self
                .agents
                .get(&session.0.clone())
                .ok_or(anyhow::Error::msg("Agent not found"))?;

            let process = SessionProcess::new(
                session.clone(),
                agent_config.clone(),
                agent.clone(),
                transport.clone(),
            );

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
    async fn lobotomy(&mut self) {
        self.last_interact_at = Utc::now();
        self.session_memory.flush();
    }

    async fn is_stale(&self) -> bool {
        let diff = Utc::now() - self.last_interact_at;

        diff.to_std().unwrap() > self.session_ttl
    }
}

struct SessionProcess {
    session_transport: Arc<SessionTransport>,
    handle: JoinHandle<()>,
}

impl SessionProcess {
    fn new(
        session: VizierSession,
        agent_config: AgentConfig,
        agent: Arc<VizierAgent>,
        vizier_transport: VizierTransport,
    ) -> Self {
        let session_transport = Arc::new(flume::unbounded());
        let send_response = async move |response| {
            let res = vizier_transport
                .send_response(session.clone(), response)
                .await;

            res
        };

        Self {
            session_transport: session_transport.clone(),
            handle: tokio::spawn(async move {
                let session = Arc::new(Mutex::new(AgentSession {
                    session_memory: SessionMemories::new(agent_config.memory.clone()),
                    session_ttl: *agent_config.session_ttl,
                    last_interact_at: Utc::now(),
                }));

                let main_session = session.clone();
                let main_handler = tokio::spawn(async move {
                    let send_response = send_response.clone();
                    while let Ok(request) = session_transport.1.recv_async().await {
                        let send_lobotomy = send_response.clone();
                        if request.content == "/lobotomy" {
                            let _ = main_session.lock().await.lobotomy().await;
                            tokio::spawn(async move {
                                if let Err(err) =
                                    send_lobotomy(VizierResponse::Message("YIPEEEE".into())).await
                                {
                                    log::error!("{}", err);
                                }
                            });

                            continue;
                        }

                        if request.is_silent_read {
                            let _ = agent
                                .handle_silent_read(main_session.lock().await, &request)
                                .await;
                            continue;
                        }

                        let send_thinking = send_response.clone();
                        let thinking = tokio::spawn(async move {
                            loop {
                                let _ = send_thinking(VizierResponse::Thinking).await;

                                tokio::time::sleep(Duration::from_secs(5)).await;
                            }
                        });

                        let content = agent.handle_chat(&request, main_session.lock().await).await;
                        let send_response = send_response.clone();
                        match content {
                            Err(err) => {
                                if let Err(err) =
                                    send_response(VizierResponse::Message(err.to_string())).await
                                {
                                    log::error!("{}", err);
                                }
                            }
                            Ok(content) => {
                                if let Err(err) =
                                    send_response(VizierResponse::Message(content)).await
                                {
                                    log::error!("{}", err);
                                }
                            }
                        }

                        thinking.abort();
                    }
                });

                let session = session.clone();
                let stale_handler = tokio::spawn(async move {
                    loop {
                        let session = session.lock().await;
                        if session.is_stale().await {
                            log::debug!("{} session stale", agent_config.name);
                            main_handler.abort();
                            return;
                        }

                        let _ = tokio::time::sleep(session.session_ttl).await;
                    }
                });

                let _ = stale_handler.await;
            }),
        }
    }
}
