use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::agent::agent_impl::VizierAgent;
use crate::agent::memory::SessionMemories;
use crate::agent::session::VizierSession;
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::transport::{VizierRequest, VizierResponse};
use crate::utils::remove_think_tags;

pub mod agent_impl;
pub mod exec;
pub mod memory;
pub mod session;
pub mod tools;

#[derive(Clone)]
pub struct VizierAgents {
    deps: VizierDependencies,
    sessions: Arc<Mutex<HashMap<VizierSession, AgentSession>>>,
    agents: Arc<HashMap<String, VizierAgent>>,
}

impl VizierAgents {
    pub async fn new(deps: VizierDependencies) -> Result<Self> {
        let mut agents = HashMap::new();

        let config = deps.config.clone();
        for (agent_id, _) in config.agents.iter() {
            agents.insert(
                agent_id.clone(),
                VizierAgent::new(&mut deps.clone(), agent_id.clone())?,
            );
        }

        Ok(Self {
            deps,
            agents: Arc::new(agents),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let sessions = self.sessions.clone();

        let cleanup_sessions = self.sessions.clone();
        // stale agent session killer
        let cleanup_handle = tokio::spawn(async move {
            loop {
                let lookup = cleanup_sessions.lock().await.clone();
                for (session, agent_session) in lookup.iter() {
                    if agent_session.is_stale().await {
                        sessions.lock().await.remove(session);
                    }
                }
            }
        });

        let transport = self.deps.transport.clone();
        while let Ok((session, request)) = transport.read_request().await {
            // handle user requested lobotomy
            let lobotomy_transport = self.deps.transport.clone();
            if request.content == "/lobotomy" {
                let _ = self.handle_lobotomy(&session).await;
                tokio::spawn(async move {
                    if let Err(err) = lobotomy_transport
                        .send_response(session.clone(), VizierResponse::Message("YIPEEEE".into()))
                        .await
                    {
                        log::error!("{}", err);
                    }
                });

                continue;
            }

            if request.is_silent_read {
                self.handle_silent_read(&session, &request).await?;
                continue;
            }

            // start thinking every 5 second until response ready
            let thinking_transport = transport.clone();
            let thinking_session = session.clone();
            let thinking = tokio::spawn(async move {
                loop {
                    let _ = thinking_transport
                        .send_response(thinking_session.clone(), VizierResponse::Thinking)
                        .await;

                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            });

            let content = self.handle_chat(&session.clone(), &request).await;
            match content {
                Err(err) => {
                    if let Err(err) = transport
                        .send_response(session.clone(), VizierResponse::Message(err.to_string()))
                        .await
                    {
                        log::error!("{}", err);
                    }
                }
                Ok(content) => {
                    if let Err(err) = transport
                        .send_response(session.clone(), VizierResponse::Message(content))
                        .await
                    {
                        log::error!("{}", err);
                    }
                }
            }

            // stop thinking
            thinking.abort();
        }

        cleanup_handle.abort();
        Ok(())
    }

    fn init_session(&self, VizierSession(agent_id, _): &VizierSession) -> Result<AgentSession> {
        let agent_config = self.deps.config.agents.get(agent_id);

        if let Some(agent_config) = agent_config {
            Ok(AgentSession {
                session_memory: SessionMemories::new(agent_config.memory.clone()),
                session_ttl: *agent_config.session_ttl,
                last_interact_at: Utc::now(),
            })
        } else {
            Err(VizierError("Agent not found".into()).into())
        }
    }

    async fn handle_silent_read(
        &mut self,
        session: &VizierSession,
        req: &VizierRequest,
    ) -> Result<()> {
        if let Some(agent) = self.agents.get(&session.0) {
            if let Some(agent_session) = self.sessions.lock().await.get_mut(session) {
                agent_session.silent_read(req.clone(), agent).await?
            } else {
                let mut agent_session = self.init_session(session)?;
                agent_session.silent_read(req.clone(), agent).await?;
                self.sessions
                    .lock()
                    .await
                    .insert(session.clone(), agent_session);
            }

            Ok(())
        } else {
            Err(VizierError("Agent not found".into()).into())
        }
    }

    async fn handle_chat(
        &mut self,
        session: &VizierSession,
        req: &VizierRequest,
    ) -> Result<String> {
        if let Some(agent) = self.agents.get(&session.0) {
            // find session, if none found, make it then retry
            if let Some(agent_session) = self.sessions.lock().await.get_mut(session) {
                agent_session.chat(req.clone(), agent).await
            } else {
                let mut agent_session = self.init_session(session)?;
                let response = agent_session.chat(req.clone(), agent).await?;
                self.sessions
                    .lock()
                    .await
                    .insert(session.clone(), agent_session.clone());

                Ok(response)
            }
        } else {
            Err(VizierError("Agent not found".into()).into())
        }
    }

    async fn handle_lobotomy(&mut self, session: &VizierSession) -> Result<()> {
        // find session, if none found, make it then retry
        if let Some(agent_session) = self.sessions.lock().await.get_mut(session) {
            agent_session.lobotomy().await;
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
    async fn chat(&mut self, req: VizierRequest, agent: &VizierAgent) -> Result<String> {
        let response = agent.chat(req.clone(), &self.session_memory).await?;

        let response_msg = remove_think_tags(&*response);

        self.session_memory.push_user_message(req.clone());
        self.session_memory.push_agent(response_msg);
        self.session_memory.try_summarize(&agent).await?;

        self.last_interact_at = Utc::now();
        Ok(response.to_string())
    }

    async fn silent_read(&mut self, req: VizierRequest, agent: &VizierAgent) -> Result<()> {
        agent.silent_read(req.clone(), &self.session_memory).await?;

        self.session_memory.push_user_message(req.clone());
        self.session_memory.try_summarize(agent).await?;

        self.last_interact_at = Utc::now();
        Ok(())
    }

    async fn lobotomy(&mut self) {
        self.last_interact_at = Utc::now();
        self.session_memory.flush();
    }

    async fn is_stale(&self) -> bool {
        let diff = Utc::now() - self.last_interact_at;

        diff.to_std().unwrap() > self.session_ttl
    }
}
