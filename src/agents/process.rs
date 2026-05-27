use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use chrono::Utc;
use rig::message::Message;
use tokio::task::{JoinHandle, JoinSet};

use crate::{
    agents::{
        agent::{VizierAgent, read_md_file},
        hook::{
            VizierSessionHooks, debug::DebugHook, history::HistoryHook, thinking::ThinkingHook,
            tool_calls::ToolCallsHook,
        },
    },
    config::agent::AgentConfig,
    dependencies::VizierDependencies,
    error::VizierError,
    schema::{
        AgentId, VizierChannelId, VizierRequest, VizierRequestContent, VizierResponse,
        VizierResponseContent, VizierSession, VizierSessionDetail,
    },
    storage::{
        VizierStorage, history::HistoryStorage, memory::MemoryStorage, session::SessionStorage,
    },
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
    let mut detail_tasks = JoinSet::new();

    let heartbeat_agent_id = agent_id.clone();
    let heartbeat_interval = *agent_config.heartbeat_interval.clone();
    let heartbeat_transport = deps.transport.clone();
    let heartbeat_workspace = agent.workspace.clone();
    let heartbeat = tokio::spawn(async move {
        let mut interval = tokio::time::interval(heartbeat_interval);
        loop {
            interval.tick().await;
            let now = Utc::now();
            let session = VizierSession(
                heartbeat_agent_id.clone(),
                VizierChannelId::Heartbeat(now.clone()),
                None,
            );

            let prompt = read_md_file(heartbeat_workspace.clone(), "HEARTBEAT.md".into());
            if !prompt.is_empty() {
                if let Err(err) = heartbeat_transport
                    .send_request(
                        session,
                        VizierRequest {
                            timestamp: Utc::now(),
                            user: heartbeat_agent_id.clone(),
                            content: VizierRequestContent::Task(prompt.clone()),
                            metadata: serde_json::json!({}),

                            ..Default::default()
                        },
                    )
                    .await
                {
                    tracing::error!("heartbeat error: {}", err);
                }
            }
        }
    });

    let dream_agent_id = agent_id.clone();
    let dream_interval = *agent_config.dream_interval.clone();
    let dream_transport = deps.transport.clone();
    let dream_storage = deps.storage.clone();
    let dream = tokio::spawn(async move {
        let prompt = "Extract, summarize, and/or analyze previous conversations.
        and do the following:
        1. adjust your AGENT.md, IDENTITY.md, HEARTBEAT.md accordingly.
        2. make one or multiple mamories about them, if you need them long term.
        "
        .to_string();

        let mut interval = tokio::time::interval(dream_interval);
        let _ = interval.tick().await; // skip when first run

        loop {
            interval.tick().await;

            // collect all sessions
            if let Ok(sessions) = dream_storage
                .get_session_list(dream_agent_id.clone(), None)
                .await
            {
                let sessions = sessions
                    .iter()
                    .filter_map(|session| match &session.channel {
                        VizierChannelId::System
                        | VizierChannelId::Subagent
                        | VizierChannelId::Dream(_)
                        | VizierChannelId::Task(_, _)
                        | VizierChannelId::InterAgent(_) => None,

                        // only reflect on user intection channel
                        channel => Some(VizierSession(
                            session.agent_id.clone(),
                            channel.clone(),
                            session.topic.clone(),
                        )),
                    });

                let agent_id = dream_agent_id.clone();
                let now = Utc::now();
                for session in sessions {
                    if let Err(err) = dream_transport
                        .send_request(
                            VizierSession(
                                agent_id.clone(),
                                VizierChannelId::Dream(Box::new(session)),
                                Some(now.to_rfc3339()),
                            ),
                            VizierRequest {
                                timestamp: now,
                                user: agent_id.clone(),
                                content: VizierRequestContent::Task(prompt.clone()),
                                metadata: serde_json::json!({}),

                                ..Default::default()
                            },
                        )
                        .await
                    {
                        tracing::error!("dream error: {}", err);
                    }
                }
            }
        }
    });

    while let Ok((session, request)) = recv.recv().await {
        if session.0 != agent_id {
            continue;
        }

        // handle session_detail creator
        let session_detail_storage = deps.storage.clone();
        let session_detail_session = session.clone();
        let session_detail_agent = agent.clone();
        let session_detail_request = request.clone();
        detail_tasks.spawn(async move {
            let agent_id = session_detail_session.0;
            let channel = session_detail_session.1;
            let topic = session_detail_session.2;

            if let None = session_detail_storage
                .get_session_detail_by_topic(agent_id.clone(), channel.clone(), topic.clone())
                .await
                .unwrap_or(None)
            {
                let prompt = format!(
                    r#"summarize (don't execute) the prompt below into a 60 character title: 
"{}"

**only response the summarize title**"#,
                    session_detail_request.to_prompt().unwrap()
                );
                let res = session_detail_agent
                    .prompt(Message::system(prompt), vec![], 0, None, false)
                    .await;
                if let Ok((title, _)) = res {
                    let mut title = title.clone();
                    title.truncate(60);

                    if title.starts_with('"') {
                        title.remove(0);
                    }

                    if title.ends_with('"') {
                        title.pop();
                    }

                    let detail = VizierSessionDetail {
                        agent_id,
                        channel,
                        topic,
                        title,
                    };
                    let _ = session_detail_storage.save_session_detail(detail).await;
                }
            }
        });

        if let Some(handle) = main_handles.get(&session) {
            if !handle.is_finished() {
                let _ = deps
                    .transport
                    .send_response(
                        session.clone(),
                        crate::schema::VizierResponse {
                            timestamp: chrono::Utc::now(),
                            content: crate::schema::VizierResponseContent::Abort,
                            attachments: vec![],
                        },
                    )
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
                let _ = thinking_transport
                    .send_response(
                        thinking_session.clone(),
                        crate::schema::VizierResponse {
                            timestamp: chrono::Utc::now(),
                            content: crate::schema::VizierResponseContent::ThinkingStart,
                            attachments: vec![],
                        },
                    )
                    .await;
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
                    tracing::error!("{}", err);
                    let _ = transport
                        .send_response(
                            session.clone(),
                            crate::schema::VizierResponse {
                                timestamp: chrono::Utc::now(),
                                content: crate::schema::VizierResponseContent::Message {
                                    content: format!("ERR: {}", err),
                                    stats: None,
                                },
                                attachments: vec![],
                            },
                        )
                        .await;
                }

                thinking_handle.abort();
            }),
        );
    }

    heartbeat.abort();
    dream.abort();

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
        VizierRequestContent::Chat(prompt) => {
            let history = storage
                .list_session_history(
                    session.clone(),
                    Some(request.timestamp.clone()),
                    Some(agent_config.session_memory.max_capacity),
                )
                .await?;

            let memory = storage
                .query_memory(session.0.clone(), prompt.clone(), 10, 0.5)
                .await?;
            let res = agent.chat(request, history, memory, Some(hooks)).await?;
            transport.send_response(session, res).await?;
        }
        VizierRequestContent::SilentRead(_) => {
            let history = storage
                .list_session_history(
                    session.clone(),
                    Some(request.timestamp.clone()),
                    Some(agent_config.session_memory.max_capacity),
                )
                .await?;
            let res = agent.chat(request, history, vec![], Some(hooks)).await?;
            transport.send_response(session, res).await?;
        }
        VizierRequestContent::Prompt(_) | VizierRequestContent::Task(_) => {
            let history = match &session.1 {
                VizierChannelId::Dream(dream_session) => {
                    let dream_interval = *agent_config.dream_interval;
                    let end = request.timestamp.clone();
                    let start = end - dream_interval;
                    let history = storage
                        .list_session_by_time_window(*dream_session.clone(), Some(start), Some(end))
                        .await?;

                    // skip dreaming if history is empty
                    if history.is_empty() {
                        let res = VizierResponse {
                            timestamp: end.clone(),
                            content: VizierResponseContent::Abort,
                            attachments: vec![],
                        };

                        transport.send_response(session, res).await?;
                        return Ok(());
                    }

                    history
                }
                _ => vec![],
            };

            let res = agent.chat(request, history, vec![], Some(hooks)).await?;
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

        if let Some(true) = agent_config.show_tool_calls {
            hooks = hooks.hook(ToolCallsHook::new(deps.transport.clone(), session.clone()));
        }

        let hooks = Arc::new(hooks);

        Ok(Self { hooks })
    }
}
