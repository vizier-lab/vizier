use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;
use chrono::Utc;
use tokio::task::JoinHandle;

use crate::{
    agents::{
        agent::VizierAgent,
        hook::{
            VizierSessionHooks, debug::DebugHook, history::HistoryHook, thinking::ThinkingHook,
        },
    },
    config::agent::AgentConfig,
    dependencies::VizierDependencies,
    error::VizierError,
    schema::{
        AgentId, VizierRequest, VizierRequestContent, VizierResponse, VizierSession,
        VizierSessionDetail,
    },
    storage::{VizierStorage, history::HistoryStorage, session::SessionStorage},
    transport::VizierTransport,
};

pub async fn agent_process(agent_id: AgentId, deps: VizierDependencies) -> Result<()> {
    let agent = Arc::new(VizierAgent::new(agent_id.clone(), &deps).await?);
    let agent_config = deps
        .config
        .agents
        .get(&agent_id)
        .ok_or(VizierError("agent is not found".into()))?;

    let mut recv = deps.transport.subscribe_request().await?;
    let mut agent_sessions = HashMap::<VizierSession, AgentSession>::new();

    let mut main_handles = HashMap::<VizierSession, JoinHandle<()>>::new();
    let mut thinking_handles = HashMap::<VizierSession, Arc<JoinHandle<()>>>::new();

    while let Ok((session, request)) = recv.recv().await {
        if session.0 != agent_id {
            continue;
        }

        // handle session_detail creator
        let session_detail_storage = deps.storage.clone();
        let session_detail_session = session.clone();
        let session_detail_agent = agent.clone();
        let session_detail_request = request.clone();
        tokio::spawn(async move {
            let agent_id = session_detail_session.0;
            let channel = session_detail_session.1;
            let topic = session_detail_session.2;

            if let None = session_detail_storage
                .get_session_detail_by_topic(agent_id.clone(), channel.clone(), topic.clone())
                .await
                .unwrap_or(None)
            {
                let res = session_detail_agent
                    .chat(
                        VizierRequest {
                            user: "system".into(),
                            content: VizierRequestContent::Prompt(format!(
                                "summarize prompt below into a single line title: \n {}",
                                session_detail_request.to_prompt().unwrap()
                            )),
                            metadata: serde_json::json!({}),
                        },
                        vec![],
                        None,
                    )
                    .await;
                if let Ok(VizierResponse::Message { content, stats: _ }) = res {
                    let detail = VizierSessionDetail {
                        agent_id,
                        channel,
                        topic,
                        title: content,
                    };
                    let _ = session_detail_storage.save_session_detail(detail).await;
                }
            }
        });

        if let Some(handle) = main_handles.get(&session) {
            if !handle.is_finished() {
                let _ = deps
                    .transport
                    .send_response(session.clone(), crate::schema::VizierResponse::Abort)
                    .await;
            }
            handle.abort();
        }

        let agent_session = if let Some(agent_session) = agent_sessions.get(&session) {
            agent_session.clone()
        } else {
            let agent_session =
                AgentSession::new(agent_config.clone(), session.clone(), deps.clone())?;
            agent_sessions.insert(session.clone(), agent_session.clone());

            agent_session
        };

        // handle thinking
        if let Some(handle) = thinking_handles.get(&session) {
            handle.abort();
        }
        let thinking_transport = deps.transport.clone();
        let thinking_request = request.clone();
        let thinking_session = session.clone();
        let thinking_handle = Arc::new(tokio::spawn(async move {
            if let VizierRequestContent::Chat(_) = thinking_request.content {
                loop {
                    let _ = thinking_transport
                        .send_response(
                            thinking_session.clone(),
                            crate::schema::VizierResponse::ThinkingProgress,
                        )
                        .await;

                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }));
        thinking_handles.insert(session.clone(), thinking_handle.clone());

        let agent = agent.clone();
        let agent_config = agent_config.clone();
        let session = session.clone();
        let transport = deps.transport.clone();
        let storage = deps.storage.clone();
        main_handles.insert(
            session.clone(),
            tokio::spawn(async move {
                if let Err(err) = handle_request(
                    agent.clone(),
                    agent_config.clone(),
                    session.clone(),
                    request.clone(),
                    transport.clone(),
                    storage.clone(),
                    agent_session.hooks.clone(),
                )
                .await
                {
                    log::error!("{}", err);
                    let _ = transport
                        .send_response(
                            session.clone(),
                            crate::schema::VizierResponse::Message {
                                content: format!("ERR: {}", err),
                                stats: None,
                            },
                        )
                        .await;
                }

                thinking_handle.abort();
            }),
        );
    }

    Ok(())
}

pub async fn handle_request(
    agent: Arc<VizierAgent>,
    agent_config: AgentConfig,
    session: VizierSession,
    request: VizierRequest,
    transport: VizierTransport,
    storage: Arc<VizierStorage>,
    hooks: Arc<VizierSessionHooks>,
) -> Result<()> {
    match &request.content {
        VizierRequestContent::Chat(_) | VizierRequestContent::SilentRead(_) => {
            let history = storage
                .list_session_history(
                    session.clone(),
                    Some(Utc::now()),
                    Some(agent_config.session_memory.max_capacity),
                )
                .await?;

            let res = agent.chat(request, history, Some(hooks)).await?;
            transport.send_response(session, res).await?;
        }
        VizierRequestContent::Prompt(_) | VizierRequestContent::Task(_) => {
            let res = agent.chat(request, vec![], Some(hooks)).await?;
            transport.send_response(session, res).await?;
        }
        VizierRequestContent::Command(_) => unimplemented!(),
    }

    Ok(())
}

#[derive(Clone)]
pub struct AgentSession {
    hooks: Arc<VizierSessionHooks>,
}

impl AgentSession {
    pub fn new(
        agent_config: AgentConfig,
        session: VizierSession,
        deps: VizierDependencies,
    ) -> Result<Self> {
        let mut hooks = VizierSessionHooks::new()
            .hook(DebugHook(session.clone()))
            .hook(HistoryHook::new(deps.storage.clone(), session.clone()));

        if let Some(true) = agent_config.show_thinking {
            hooks = hooks.hook(ThinkingHook::new(deps.transport.clone(), session.clone()));
        }
        let hooks = Arc::new(hooks);

        Ok(Self { hooks })
    }
}
