use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use rig_core::message::Message;
use tokio::sync::{mpsc, watch};
use tokio::task::{JoinHandle, JoinSet};

use crate::{
    agents::{
        agent::{VizierAgent, read_md_file},
        hook::{
            VizierSessionHooks, debug::DebugHook, history::HistoryHook, thinking::ThinkingHook,
            tool_calls::ToolCallsHook,
        },
    },
    dependencies::VizierDependencies,
    schema::{
        AgentConfig, AgentId, VizierChannelId, VizierRequest, VizierRequestContent, VizierResponse,
        VizierResponseContent, VizierSession, VizierSessionDetail,
    },
    storage::{
        VizierStorage, history::HistoryStorage, memory::MemoryStorage, session::SessionStorage,
    },
};

pub async fn agent_process(
    agent_id: AgentId,
    deps: VizierDependencies,
    agent_config: AgentConfig,
    _shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    let agent = Arc::new(VizierAgent::new(agent_id.clone(), &deps, &agent_config).await?);

    let recv = deps.transport.register_agent(agent_id.clone()).await;

    let mut main_handles = HashMap::<VizierSession, JoinHandle<()>>::new();
    let mut thinking_handles = HashMap::<VizierSession, Arc<JoinHandle<()>>>::new();
    let mut detail_tasks = JoinSet::new();

    let heartbeat_agent_id = agent_id.clone();
    let heartbeat_interval = *agent_config.heartbeat_interval;
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
                        None,
                    )
                    .await
                {
                    tracing::error!("heartbeat error: {}", err);
                }
            }
        }
    });

    let dream_agent_id = agent_id.clone();
    let dream_interval = *agent_config.dream_interval;
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
                            None,
                        )
                        .await
                    {
                        tracing::error!("dream error: {}", err);
                    }
                }
            }
        }
    });

    let mut session_queues = HashMap::<VizierSession, VecDeque<VizierRequest>>::new();
    let mut message_counts = HashMap::<VizierSession, usize>::new();
    let (complete_tx, mut complete_rx) = mpsc::unbounded_channel::<VizierSession>();

    loop {
        tokio::select! {
            result = recv.recv_async() => {
                let Ok(envelope) = result else { break };
                let session = envelope.session;
                let request = envelope.request;
                let response_tx = envelope.response_tx;

                // handle session_detail
                let session_detail_storage = deps.storage.clone();
                let session_detail_session = session.clone();
                let session_detail_agent = agent.clone();
                let session_detail_request = request.clone();
                let msg_count = message_counts.entry(session.clone()).or_insert(0);
                *msg_count += 1;
                let current_count = *msg_count;
                detail_tasks.spawn(async move {
                    let agent_id = session_detail_session.0;
                    let channel = session_detail_session.1;
                    let topic = session_detail_session.2;
                    let slug_title = topic.clone().unwrap_or("DEFAULT".to_string());

                    if current_count == 1 {
                        // Create session_detail immediately with slug as title
                        let detail = VizierSessionDetail {
                            agent_id,
                            channel,
                            topic,
                            title: slug_title,
                            is_thinking: false,
                        };
                        let _ = session_detail_storage.save_session_detail(detail).await;
                    } else if current_count == 10 {
                        // Check if title is still the slug (hasn't been updated yet)
                        if let Ok(Some(existing)) = session_detail_storage
                            .get_session_detail_by_topic(agent_id.clone(), channel.clone(), topic.clone())
                            .await
                        {
                            if existing.title == slug_title {
                                // Generate title via LLM
                                let prompt = format!(
                                    r#"summarize (don't execute) the prompt below into a 60 character title:
"{}"

**only response the summarize title**"#,
                                    session_detail_request.to_prompt().unwrap()
                                );
                                let res = session_detail_agent
                                    .prompt(Message::user(prompt), vec![], 0, None, false)
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
                                        is_thinking: false,
                                    };
                                    let _ = session_detail_storage.update_session_detail(detail).await;
                                }
                            }
                        }
                    }
                });

                // Handle abort command
                if let VizierRequestContent::Command(ref cmd) = request.content {
                    if cmd == "abort" {
                        // Abort active task for this session
                        if let Some(handle) = main_handles.get(&session) {
                            if !handle.is_finished() {
                                handle.abort();
                                if let Some(ref tx) = response_tx {
                                    let _ = tx
                                        .send_async(VizierResponse {
                                            timestamp: chrono::Utc::now(),
                                            content: crate::schema::VizierResponseContent::Abort,
                                            attachments: vec![],
                                        })
                                        .await;
                                }
                            }
                        }
                        // Abort thinking indicator
                        if let Some(handle) = thinking_handles.remove(&session) {
                            handle.abort();
                        }
                        // Clear queued messages
                        session_queues.remove(&session);
                        continue;
                    }
                }

                // Queue message if a task is already running for this session
                if let Some(handle) = main_handles.get(&session) {
                    if !handle.is_finished() {
                        session_queues.entry(session.clone()).or_default().push_back(request);
                        continue;
                    }
                }

                // handle thinking
                if let Some(handle) = thinking_handles.get(&session) {
                    handle.abort();
                }
                let thinking_response_tx = response_tx.clone();
                let thinking_request = request.clone();
                let thinking_session = session.clone();
                let thinking_handle = Arc::new(tokio::spawn(async move {
                    if let VizierRequestContent::Chat(_) = thinking_request.content {
                        if let Some(ref tx) = thinking_response_tx {
                            let _ = tx
                                .send_async(VizierResponse {
                                    timestamp: chrono::Utc::now(),
                                    content: crate::schema::VizierResponseContent::ThinkingStart,
                                    attachments: vec![],
                                })
                                .await;
                        }
                    }
                }));
                thinking_handles.insert(session.clone(), thinking_handle.clone());

                let agent = agent.clone();
                let agent_config = agent_config.clone();
                let session = session.clone();
                let storage = deps.storage.clone();
                let complete_tx = complete_tx.clone();
                let thinking_storage = storage.clone();
                let thinking_session = session.clone();
                main_handles.insert(
                    session.clone(),
                    tokio::spawn(async move {
                        // Set is_thinking = true
                        let _ = thinking_storage
                            .update_thinking_state(
                                thinking_session.0.clone(),
                                thinking_session.1.clone(),
                                thinking_session.2.clone(),
                                true,
                            )
                            .await;

                        if let Err(err) = handle_request(
                            agent.clone(),
                            agent_config.clone(),
                            session.clone(),
                            request.clone(),
                            response_tx.clone(),
                            storage.clone(),
                        )
                        .await
                        {
                            tracing::error!("{}", err);
                            if let Some(ref tx) = response_tx {
                                let _ = tx
                                    .send_async(VizierResponse {
                                        timestamp: chrono::Utc::now(),
                                        content: crate::schema::VizierResponseContent::Message {
                                            content: format!("ERR: {}", err),
                                            stats: None,
                                        },
                                        attachments: vec![],
                                    })
                                    .await;
                            }
                        }

                        thinking_handle.abort();

                        // Set is_thinking = false
                        let _ = storage
                            .update_thinking_state(
                                session.0.clone(),
                                session.1.clone(),
                                session.2.clone(),
                                false,
                            )
                            .await;

                        let _ = complete_tx.send(session);
                    }),
                );
            }
            // Handle task completions — process next queued message
            Some(completed_session) = complete_rx.recv() => {
                if let Some(queue) = session_queues.get_mut(&completed_session) {
                    if let Some(next_request) = queue.pop_front() {
                        // handle thinking
                        if let Some(handle) = thinking_handles.get(&completed_session) {
                            handle.abort();
                        }
                        let thinking_request = next_request.clone();
                        let thinking_session = completed_session.clone();
                        let thinking_handle = Arc::new(tokio::spawn(async move {
                            if let VizierRequestContent::Chat(_) = thinking_request.content {
                                tracing::info!("thinking started for {:?}", thinking_session);
                            }
                        }));
                        thinking_handles.insert(completed_session.clone(), thinking_handle.clone());

                        let agent = agent.clone();
                        let agent_config = agent_config.clone();
                        let session = completed_session.clone();
                        let storage = deps.storage.clone();
                        let complete_tx = complete_tx.clone();
                        let thinking_storage = storage.clone();
                        let thinking_session = session.clone();
                        main_handles.insert(
                            session.clone(),
                            tokio::spawn(async move {
                                // Set is_thinking = true
                                let _ = thinking_storage
                                    .update_thinking_state(
                                        thinking_session.0.clone(),
                                        thinking_session.1.clone(),
                                        thinking_session.2.clone(),
                                        true,
                                    )
                                    .await;

                                if let Err(err) = handle_request(
                                    agent.clone(),
                                    agent_config.clone(),
                                    session.clone(),
                                    next_request.clone(),
                                    None,
                                    storage.clone(),
                                )
                                .await
                                {
                                    tracing::error!("{}", err);
                                }

                                thinking_handle.abort();

                                // Set is_thinking = false
                                let _ = storage
                                    .update_thinking_state(
                                        session.0.clone(),
                                        session.1.clone(),
                                        session.2.clone(),
                                        false,
                                    )
                                    .await;

                                let _ = complete_tx.send(session);
                            }),
                        );
                    }
                }
            }
        }
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
    response_tx: Option<flume::Sender<VizierResponse>>,
    storage: Arc<VizierStorage>,
) -> Result<()> {
    let mut hooks = VizierSessionHooks::new()
        .hook(DebugHook(session.clone()))
        .hook(HistoryHook::new(storage.clone(), session.clone()));

    if let Some(true) = agent_config.show_thinking {
        if let Some(ref tx) = response_tx {
            hooks = hooks.hook(ThinkingHook::new(tx.clone(), session.clone()));
        }
    }

    if let Some(true) = agent_config.show_tool_calls {
        if let Some(ref tx) = response_tx {
            hooks = hooks.hook(ToolCallsHook::new(tx.clone(), session.clone()));
        }
    }

    let hooks = Arc::new(hooks);

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
            if let Some(ref tx) = response_tx {
                let _ = tx.send_async(res).await;
            }
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
            if let Some(ref tx) = response_tx {
                let _ = tx.send_async(res).await;
            }
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
                        if let Some(ref tx) = response_tx {
                            let res = VizierResponse {
                                timestamp: end.clone(),
                                content: VizierResponseContent::Abort,
                                attachments: vec![],
                            };
                            let _ = tx.send_async(res).await;
                        }
                        return Ok(());
                    }

                    history
                }
                _ => vec![],
            };

            let res = agent.chat(request, history, vec![], Some(hooks)).await?;
            if let Some(ref tx) = response_tx {
                let _ = tx.send_async(res).await;
            }
        }
        VizierRequestContent::Command(cmd) => {
            tracing::warn!("unhandled command: {}", cmd);
        }
        VizierRequestContent::Reaction(event) => {
            log::info!(
                "Reaction recorded: user={}, emoji={}, action={}, message={:?}",
                event.user_id,
                event.emoji,
                event.action_str(),
                event.platform_message_id
            );
        }
    }

    Ok(())
}
