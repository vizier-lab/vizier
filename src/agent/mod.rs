use anyhow::Result;
use chrono::Utc;
use flume::{Receiver, Sender};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, MutexGuard};
use tokio::task::JoinHandle;

use crate::agent::agent_impl::VizierAgent;
use crate::agent::hook::VizierSessionHooks;
use crate::agent::hook::debug::DebugHook;
use crate::agent::hook::history::HistoryHook;
use crate::agent::hook::thinking::ThinkingHook;
use crate::agent::memory::SessionMemories;
use crate::config::agent::AgentConfig;
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::{SessionHistoryContent, VizierRequest, VizierResponse, VizierSession};
use crate::storage::history::HistoryStorage;

pub mod agent_impl;
pub mod hook;
pub mod memory;
pub mod tools;

#[derive(Clone)]
pub struct VizierAgents {
    deps: VizierDependencies,
    agents: HashMap<String, (AgentConfig, Arc<VizierAgent>)>,
}

impl VizierAgent {
    async fn handle_silent_read(
        &self,
        mut session: MutexGuard<'_, AgentSession>,
        request: &VizierRequest,
        hooks: Arc<VizierSessionHooks>,
    ) -> Result<()> {
        self.silent_read(request.clone(), &session.session_memory, hooks)
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
        hooks: Arc<VizierSessionHooks>,
    ) -> Result<VizierResponse> {
        let response = self
            .chat(request.clone(), &session.session_memory, hooks)
            .await?;

        session.session_memory.push_user_message(request.clone());
        session.session_memory.push_agent_message(response.clone());
        session.session_memory.try_summarize(&self).await?;

        session.last_interact_at = Utc::now();
        Ok(response)
    }
}

type SessionTransport = (Sender<VizierRequest>, Receiver<VizierRequest>);

impl VizierAgents {
    pub async fn new(deps: VizierDependencies) -> Result<Self> {
        let mut agents = HashMap::new();
        for (agent_id, agent_config) in deps.config.agents.iter() {
            agents.insert(
                agent_id.clone(),
                (
                    agent_config.clone(),
                    Arc::new(VizierAgent::new(agent_id.clone(), &deps).await?),
                ),
            );
        }

        Ok(Self { deps, agents })
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

            let (agent_config, agent) = self
                .agents
                .get(&session.0)
                .ok_or(VizierError("agent not found".into()))?;

            let process = SessionProcess::new(
                session.0.clone(),
                agent.clone(),
                agent_config.clone(),
                session.clone(),
                self.deps.clone(),
            )
            .await?;

            let _ = process.session_transport.0.send_async(request).await;

            sessions.insert(session, process);
        }

        Ok(())
    }
}

#[derive(Clone)]
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
        agent_id: String,
        agent: Arc<VizierAgent>,
        agent_config: AgentConfig,
        session: VizierSession,
        deps: VizierDependencies,
    ) -> Result<Self> {
        let transport = deps.transport.clone();
        let session_transport = Arc::new(flume::unbounded());

        let response_session = session.clone();
        let response_transport = transport.clone();
        let send_response = async move |response| {
            let res = response_transport
                .send_response(response_session, response)
                .await;

            res
        };

        let mut hooks = VizierSessionHooks::new()
            .hook(DebugHook(session.clone()))
            .hook(HistoryHook::new(deps.storage.clone(), session.clone()));

        if let Some(true) = agent_config.show_thinking {
            hooks = hooks.hook(ThinkingHook::new(transport.clone(), session.clone()));
        }
        let hooks = Arc::new(hooks);

        let mut memories = SessionMemories::new(
            agent_id.clone(),
            agent_config.session_memory.clone(),
            hooks.clone(),
        );

        // fill initial memory with previous conversation
        deps.storage
            .list_session_history(
                session.clone(),
                Some(Utc::now()),
                Some(agent_config.session_memory.max_capacity),
            )
            .await?
            .iter()
            .for_each(|history| match &history.content {
                SessionHistoryContent::Request(req) => memories.push_user_message(req.clone()),
                SessionHistoryContent::Response(content, stats) => {
                    memories.push_agent_message(VizierResponse::Message {
                        content: content.clone(),
                        stats: stats.clone(),
                    })
                }
            });

        Ok(Self {
            session_transport: session_transport.clone(),
            handle: tokio::spawn(async move {
                let agent_config = agent_config.clone();
                let agent_session = Arc::new(Mutex::new(AgentSession {
                    session_memory: memories,
                    session_ttl: *agent_config.session_timeout,
                    last_interact_at: Utc::now(),
                }));

                let main_session = agent_session.clone();
                let main_handler = tokio::spawn(async move {
                    let send_response = send_response.clone();
                    let mut curr_handle = tokio::spawn(async move {});

                    // **BEHOLD** the most overkill boolean to ever exits
                    let is_thinking = Arc::new(Mutex::new(false));

                    let send_thinking = send_response.clone();
                    let is_thinking_status = is_thinking.clone();
                    tokio::spawn(async move {
                        loop {
                            if !*is_thinking_status.lock().await {
                                continue;
                            }
                            let _ = send_thinking.clone()(VizierResponse::ThinkingProgress).await;
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                    });

                    let mut prev_req: Option<VizierRequest> = None;
                    while let Ok(request) = session_transport.1.recv_async().await {
                        let main_session = main_session.clone();

                        let send_response = send_response.clone();
                        let agent = agent.clone();
                        let hooks = hooks.clone();

                        let is_finished = curr_handle.is_finished();
                        curr_handle.abort();
                        if !is_finished {
                            if let Some(req) = &prev_req {
                                main_session
                                    .lock()
                                    .await
                                    .session_memory
                                    .push_user_message(req.clone());
                            }
                        }

                        if request.content == "/abort" {
                            continue;
                        }

                        *is_thinking.lock().await = false;
                        prev_req = Some(request.clone());

                        let handler_thinking = is_thinking.clone();
                        curr_handle = tokio::spawn(async move {
                            let mut main_session = main_session.lock().await;
                            let send_lobotomy = send_response.clone();
                            if request.content == "/lobotomy" {
                                let _ = main_session.lobotomy();
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

                                return;
                            }

                            if request.is_silent_read {
                                let _ = agent
                                    .handle_silent_read(main_session, &request, hooks.clone())
                                    .await;
                                return;
                            }

                            *handler_thinking.lock().await = true;
                            let content = agent
                                .handle_chat(&request, main_session, hooks.clone())
                                .await;
                            let send_response = send_response.clone();
                            match content {
                                Err(err) => {
                                    if let Err(err) = send_response(VizierResponse::Message {
                                        content: err.to_string(),
                                        stats: None,
                                    })
                                    .await
                                    {
                                        log::error!("{}", err);
                                    }
                                }
                                Ok(response) => {
                                    if let Err(err) = send_response(response).await {
                                        log::error!("{}", err);
                                    }
                                }
                            }
                            *handler_thinking.lock().await = false;
                        });
                    }
                });

                let agent_session = agent_session.clone();
                let session_ttl = agent_config.session_timeout;
                let stale_handler = tokio::spawn(async move {
                    loop {
                        let _ = tokio::time::sleep(*session_ttl).await;
                        let agent_session = agent_session.lock().await;
                        if agent_session.is_stale() {
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
