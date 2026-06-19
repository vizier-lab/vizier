use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use anyhow::Result;
use chrono::Utc;
use rig_core::message::Message;
use tokio::sync::{mpsc, watch};
use tokio::task::{JoinHandle, JoinSet};

use crate::{
    agents::{
        agent::{VizierAgent, read_md_file},
        hook::{
            VizierSessionHooks, debug::DebugHook, handover::HandoverSenderHook,
            thinking::ThinkingHook, tool_calls::ToolCallsHook,
        },
        tools::ToolContext,
    },
    channels::{VizierChannel, discord::DiscordChannelReader, telegram::TelegramChannelReader},
    dependencies::VizierDependencies,
    indexer::VizierIndexer,
    schema::{
        AgentConfig, AgentId, DreamStage, ErrorKind, SessionHistoryContent, VizierChannelId,
        VizierRequest, VizierRequestContent, VizierResponse, VizierResponseContent, VizierSession,
        VizierSessionDetail, dream_journal::DreamJournalEntry, history_entries_to_messages,
    },
    storage::{
        VizierStorage, dream_journal::DreamJournalStorage, history::HistoryStorage,
        memory::MemoryStorage, session::SessionStorage,
    },
    transport::DreamCommand,
};

pub async fn agent_process(
    agent_id: AgentId,
    deps: VizierDependencies,
    agent_config: AgentConfig,
    indexer: Option<crate::indexer::VizierIndexer>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    let agent =
        Arc::new(VizierAgent::new(agent_id.clone(), &deps, &agent_config, indexer.clone()).await?);

    let recv = deps.transport.register_agent(agent_id.clone()).await;

    let mut agent_channels = spawn_agent_channels(&agent_id, &agent_config, &deps).await;
    agent_channels.run().await;

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

    let mut session_queues = HashMap::<
        VizierSession,
        VecDeque<(VizierRequest, Option<flume::Sender<VizierResponse>>)>,
    >::new();
    let mut message_counts = HashMap::<VizierSession, usize>::new();
    let (complete_tx, mut complete_rx) = mpsc::unbounded_channel::<VizierSession>();

    tracing::info!(agent_id = %agent_id, "agent process loop started");

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    tracing::info!(agent_id = %agent_id, "shutdown signal received");
                    break;
                }
            }
            // Some(_) = agent_channels.tasks.join_next(), if !agent_channels.tasks.is_empty() => {
            //     tracing::warn!(agent_id = %agent_id, "channel reader task ended unexpectedly");
            // }
            result = recv.recv_async() => {
                let Ok(envelope) = result else { break };
                let session = envelope.session;
                let request = envelope.request;
                let response_tx = envelope.response_tx;
                tracing::trace!(agent_id = %session.0, channel = ?session.1, "incoming request");

                // handle session_detail
                let session_detail_storage = deps.storage.clone();
                let session_detail_session = session.clone();
                let session_detail_session_for_ctx = session.clone();
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
                            is_thinking: true,
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
                                    .prompt(Message::user(prompt), vec![], 0, None, false, &ToolContext { session: session_detail_session_for_ctx, pending_attachments: Arc::new(Mutex::new(vec![])) })
                                    .await;

                                if let Ok((title, _, _, _)) = res {
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
                                        is_thinking: existing.is_thinking,
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
                        // Save command to history for display
                        let _ = deps.storage
                            .save_session_history(
                                session.clone(),
                                SessionHistoryContent::Command(cmd.clone()),
                            )
                            .await;

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
                        // Reset is_thinking in storage
                        {
                            let storage_clone = deps.storage.clone();
                            let session_clone = session.clone();
                            tokio::spawn(async move {
                                let _ = storage_clone
                                    .update_thinking_state(
                                        session_clone.0,
                                        session_clone.1,
                                        session_clone.2,
                                        false,
                                    )
                                    .await;
                            });
                        }
                        // Clear queued messages
                        session_queues.remove(&session);
                        continue;
                    }
                }

                // Handle dream command
                if let VizierRequestContent::Command(ref cmd) = request.content {
                    if cmd == "dream" {
                        // Send "Dream cycle started" response
                        if let Some(ref tx) = response_tx {
                            let _ = tx
                                .send_async(VizierResponse {
                                    timestamp: chrono::Utc::now(),
                                    content: VizierResponseContent::Message {
                                        content: "Dream cycle started.".to_string(),
                                        stats: None,
                                    },
                                    attachments: vec![],
                                })
                                .await;
                        }
                        // Trigger dream via transport channel
                        let _ = deps
                            .transport
                            .send_dream_command(DreamCommand {
                                agent_id: agent_id.clone(),
                                cycle_id: None,
                            })
                            .await;
                        continue;
                    }
                }

                // Handle checkpoint command
                if let VizierRequestContent::Command(ref cmd) = request.content {
                    if cmd == "checkpoint" {
                        // Save command to history for display
                        let _ = deps.storage
                            .save_session_history(
                                session.clone(),
                                SessionHistoryContent::Command(cmd.clone()),
                            )
                            .await;

                        let session_clone = session.clone();
                        let agent_clone = agent.clone();
                        let storage_clone = deps.storage.clone();
                        let response_tx_clone = response_tx.clone();
                        let agent_id_clone = agent_id.clone();

                        tokio::spawn(async move {
                            // Get session history
                            let history = match storage_clone
                                .list_session_history(session_clone.clone(), None, None)
                                .await
                            {
                                Ok(h) => h,
                                Err(e) => {
                                    tracing::error!("Failed to get session history for checkpoint: {}", e);
                                    if let Some(ref tx) = response_tx_clone {
                                        let _ = tx
                                            .send_async(VizierResponse {
                                                timestamp: chrono::Utc::now(),
                                                content: VizierResponseContent::Message {
                                                    content: "Failed to create checkpoint: could not retrieve history.".to_string(),
                                                    stats: None,
                                                },
                                                attachments: vec![],
                                            })
                                            .await;
                                    }
                                    return;
                                }
                            };

                            let messages = history_entries_to_messages(&history);
                            let ctx = ToolContext {
                                session: session_clone.clone(),
                                pending_attachments: Arc::new(Mutex::new(vec![])),
                            };

                            // Generate handover
                            let handover = match agent_clone.generate_handover_message(&messages, &ctx).await {
                                Ok(h) => h,
                                Err(e) => {
                                    tracing::error!("Failed to generate handover: {}", e);
                                    if let Some(ref tx) = response_tx_clone {
                                        let _ = tx
                                            .send_async(VizierResponse {
                                                timestamp: chrono::Utc::now(),
                                                content: VizierResponseContent::Message {
                                                    content: format!("Failed to create checkpoint: {}", e),
                                                    stats: None,
                                                },
                                                attachments: vec![],
                                            })
                                            .await;
                                    }
                                    return;
                                }
                            };

                            // Save checkpoint
                            if let Err(e) = storage_clone.save_checkpoint(session_clone.clone(), handover.clone()).await {
                                tracing::error!("Failed to save checkpoint: {}", e);
                                if let Some(ref tx) = response_tx_clone {
                                    let _ = tx
                                        .send_async(VizierResponse {
                                            timestamp: chrono::Utc::now(),
                                            content: VizierResponseContent::Message {
                                                content: format!("Failed to save checkpoint: {}", e),
                                                stats: None,
                                            },
                                            attachments: vec![],
                                        })
                                        .await;
                                }
                                return;
                            }

                            // Send checkpoint response
                            if let Some(ref tx) = response_tx_clone {
                                let _ = tx
                                    .send_async(VizierResponse {
                                        timestamp: chrono::Utc::now(),
                                        content: VizierResponseContent::Checkpoint {
                                            handover,
                                        },
                                        attachments: vec![],
                                    })
                                    .await;
                            }

                            tracing::info!("Manual checkpoint created for session {:?}", session_clone);
                        });
                        continue;
                    }
                }

                // Handle lobotomy command
                if let VizierRequestContent::Command(ref cmd) = request.content {
                    if cmd == "lobotomy" {
                        // Save command to history for display
                        let _ = deps.storage
                            .save_session_history(
                                session.clone(),
                                SessionHistoryContent::Command(cmd.clone()),
                            )
                            .await;

                        let session_clone = session.clone();
                        let storage_clone = deps.storage.clone();
                        let response_tx_clone = response_tx.clone();

                        tokio::spawn(async move {
                            // Save checkpoint with no handover
                            if let Err(e) = storage_clone.save_checkpoint(session_clone.clone(), None).await {
                                tracing::error!("Failed to save lobotomy checkpoint: {}", e);
                                if let Some(ref tx) = response_tx_clone {
                                    let _ = tx
                                        .send_async(VizierResponse {
                                            timestamp: chrono::Utc::now(),
                                            content: VizierResponseContent::Message {
                                                content: format!("Failed to create lobotomy: {}", e),
                                                stats: None,
                                            },
                                            attachments: vec![],
                                        })
                                        .await;
                                }
                                return;
                            }

                            // Send checkpoint response with no handover
                            if let Some(ref tx) = response_tx_clone {
                                let _ = tx
                                    .send_async(VizierResponse {
                                        timestamp: chrono::Utc::now(),
                                        content: VizierResponseContent::Checkpoint {
                                            handover: None,
                                        },
                                        attachments: vec![],
                                    })
                                    .await;
                            }

                            tracing::info!("Lobotomy created for session {:?}", session_clone);
                        });
                        continue;
                    }
                }

                // Queue message if a task is already running for this session
                if let Some(handle) = main_handles.get(&session) {
                    if !handle.is_finished() {
                        tracing::debug!(agent_id = %session.0, "queuing message while task in progress");
                        session_queues.entry(session.clone()).or_default().push_back((request, response_tx));
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
                    if matches!(thinking_request.content, VizierRequestContent::Chat(_) | VizierRequestContent::AudioChat(_, _)) {
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
                let deps_clone = deps.clone();
                let indexer = indexer.clone();
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
                            indexer.clone(),
                            &deps_clone,
                        )
                        .await
                        {
                            tracing::error!("{}", err);
                            if let Some(ref tx) = response_tx {
                                let err_str = err.to_string();
                                let _ = tx
                                    .send_async(VizierResponse {
                                        timestamp: chrono::Utc::now(),
                                        content: VizierResponseContent::Error {
                                            kind: ErrorKind::classify(&err_str),
                                            message: err_str,
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
                    if let Some((next_request, response_tx)) = queue.pop_front() {
                        // handle thinking
                        if let Some(handle) = thinking_handles.get(&completed_session) {
                            handle.abort();
                        }
                        let thinking_response_tx = response_tx.clone();
                        let thinking_request = next_request.clone();
                        let thinking_session = completed_session.clone();
                        let thinking_handle = Arc::new(tokio::spawn(async move {
                    if matches!(thinking_request.content, VizierRequestContent::Chat(_) | VizierRequestContent::AudioChat(_, _)) {
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
                        thinking_handles.insert(completed_session.clone(), thinking_handle.clone());

                        let agent = agent.clone();
                        let agent_config = agent_config.clone();
                        let session = completed_session.clone();
                        let storage = deps.storage.clone();
                        let deps_clone = deps.clone();
                        let indexer = indexer.clone();
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
                                    response_tx.clone(),
                                    storage.clone(),
                                    indexer.clone(),
                                    &deps_clone,
                                )
                                .await
                                {
                                    tracing::error!("{}", err);
                                    if let Some(ref tx) = response_tx {
                                        let err_str = err.to_string();
                                        let _ = tx
                                            .send_async(VizierResponse {
                                                timestamp: chrono::Utc::now(),
                                                content: VizierResponseContent::Error {
                                                    kind: ErrorKind::classify(&err_str),
                                                    message: err_str,
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
                }
            }
        }
    }

    heartbeat.abort();
    agent_channels.shutdown().await;
    Ok(())
}

pub struct AgentChannel(Box<dyn VizierChannel + Sync + Send + 'static>);

#[async_trait::async_trait]
impl VizierChannel for AgentChannel {
    async fn run(&self) -> Result<()> {
        self.0.run().await
    }

    async fn shutdown(&self) -> Result<()> {
        self.0.shutdown().await
    }
}

pub struct AgentChannels {
    channels: Vec<Arc<AgentChannel>>,
    tasks: JoinSet<()>,
}

impl AgentChannels {
    async fn run(&mut self) -> Result<()> {
        for channel in self.channels.iter() {
            let mut channel = channel.clone();
            self.tasks.spawn(async move {
                if let Err(e) = channel.run().await {
                    tracing::error!("channel error: {:?}", e);
                }
            });
        }

        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        for channel in self.channels.iter() {
            channel.shutdown().await;
        }

        Ok(())
    }
}

async fn spawn_agent_channels(
    agent_id: &str,
    agent_config: &AgentConfig,
    deps: &VizierDependencies,
) -> AgentChannels {
    let mut channels = AgentChannels {
        channels: vec![],
        tasks: JoinSet::new(),
    };
    if let Some(token) = &agent_config.discord_token
        && !token.is_empty()
    {
        let agent_id_owned = agent_id.to_string();
        let token_owned = token.clone();
        let deps_owned = deps.clone();
        match DiscordChannelReader::new(agent_id_owned.clone(), token_owned, deps_owned).await {
            Ok(mut reader) => {
                let mut discord = Arc::new(AgentChannel(Box::new(reader)));

                channels.channels.push(discord.clone());
            }
            Err(e) => {
                tracing::error!("failed to create discord reader: {:?}", e);
            }
        }
    }

    if let Some(token) = &agent_config.telegram_token
        && !token.is_empty()
    {
        let agent_id_owned = agent_id.to_string();
        let token_owned = token.clone();
        let deps_owned = deps.clone();
        match TelegramChannelReader::new(agent_id_owned.clone(), token_owned, deps_owned).await {
            Ok(mut reader) => {
                let mut telegram = Arc::new(AgentChannel(Box::new(reader)));

                channels.channels.push(telegram.clone());
            }
            Err(e) => {
                tracing::error!("failed to create telegram reader: {:?}", e);
            }
        }
    }

    channels
}

pub async fn handle_request(
    agent: Arc<VizierAgent>,
    agent_config: AgentConfig,
    session: VizierSession,
    request: VizierRequest,
    response_tx: Option<flume::Sender<VizierResponse>>,
    storage: Arc<VizierStorage>,
    indexer: Option<crate::indexer::VizierIndexer>,
    deps: &VizierDependencies,
) -> Result<()> {
    let mut hooks = VizierSessionHooks::new().hook(DebugHook(session.clone()));

    if let Some(ref tx) = response_tx {
        hooks = hooks.hook(ThinkingHook::new(tx.clone(), session.clone()));
    }

    if let Some(ref tx) = response_tx {
        hooks = hooks.hook(ToolCallsHook::new(tx.clone(), session.clone()));
    }

    // Register HandoverSenderHook if response_tx is available
    if let Some(ref tx) = response_tx {
        hooks = hooks.hook(HandoverSenderHook::new(tx.clone(), session.clone()));
    }

    let hooks = Arc::new(hooks);

    match &request.content {
        VizierRequestContent::Chat(_) | VizierRequestContent::AudioChat(_, _) => {
            let prompt = match &request.content {
                VizierRequestContent::Chat(p) => p.clone(),
                VizierRequestContent::AudioChat(_, Some(text)) => text.clone(),
                VizierRequestContent::AudioChat(_, None) => "[Voice message]".to_string(),
                _ => unreachable!(),
            };
            let (history, checkpoint_handover) = storage
                .list_session_history_until_checkpoint(
                    session.clone(),
                    Some(request.timestamp.clone()),
                )
                .await?;

            let memory = match &indexer {
                Some(idx) => {
                    storage
                        .query_memory(session.0.clone(), prompt, 10, 0.5, idx)
                        .await?
                }
                None => Vec::new(),
            };
            let res = agent
                .chat(
                    request,
                    session.clone(),
                    history,
                    memory,
                    Some(hooks),
                    checkpoint_handover,
                )
                .await?;
            if let Some(ref tx) = response_tx {
                let _ = tx.send_async(res).await;
            }
        }
        VizierRequestContent::SilentRead(_) => {
            let prompt = match &request.content {
                VizierRequestContent::SilentRead(p) => p.clone(),
                _ => unreachable!(),
            };
            let (history, checkpoint_handover) = storage
                .list_session_history_until_checkpoint(
                    session.clone(),
                    Some(request.timestamp.clone()),
                )
                .await?;
            let memory = match &indexer {
                Some(idx) => {
                    storage
                        .query_memory(session.0.clone(), prompt, 10, 0.5, idx)
                        .await?
                }
                None => Vec::new(),
            };
            let res = agent
                .chat(
                    request,
                    session.clone(),
                    history,
                    memory,
                    Some(hooks),
                    checkpoint_handover,
                )
                .await?;
            if let Some(ref tx) = response_tx {
                let _ = tx.send_async(res).await;
            }
        }
        VizierRequestContent::Prompt(_)
        | VizierRequestContent::AudioPrompt(_, _)
        | VizierRequestContent::Task(_) => {
            let res = match &session.1 {
                VizierChannelId::Dream(dream_session, stage) => {
                    let dream_start = Utc::now();
                    match stage {
                        DreamStage::Extraction => {
                            let end = request.timestamp;
                            let session_history = storage
                                .list_session_by_time_window(
                                    *dream_session.clone(),
                                    None,
                                    Some(end),
                                )
                                .await?;

                            // Skip empty sessions — send Abort
                            if session_history.is_empty() {
                                if let Some(ref tx) = response_tx {
                                    let _ = tx
                                        .send_async(VizierResponse {
                                            timestamp: end,
                                            content: VizierResponseContent::Abort,
                                            attachments: vec![],
                                        })
                                        .await;
                                }
                                return Ok(());
                            }

                            let cycle_id = request
                                .metadata
                                .get("dream_cycle_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let session_context = dream_session.to_slug();

                            let response = agent
                                .dream_chat(
                                    request,
                                    session.clone(),
                                    session_history,
                                    Some(hooks.clone()),
                                    deps,
                                )
                                .await?;

                            // Save extraction as dream journal entry
                            save_dream_entry(
                                &deps.storage,
                                &session.0,
                                &cycle_id,
                                vec![session_context.clone()],
                                Some(session_context),
                                &agent_config,
                                dream_start,
                                DreamStage::Extraction,
                                &response,
                            )
                            .await;

                            response
                        }
                        DreamStage::Consolidation => {
                            let cycle_id = request
                                .metadata
                                .get("dream_cycle_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let source_sessions: Vec<String> = serde_json::from_value(
                                request
                                    .metadata
                                    .get("source_sessions")
                                    .cloned()
                                    .unwrap_or(serde_json::json!([])),
                            )
                            .unwrap_or_default();

                            let response = agent
                                .dream_chat(
                                    request,
                                    session.clone(),
                                    vec![],
                                    Some(hooks.clone()),
                                    deps,
                                )
                                .await?;

                            // Save consolidation as dream journal entry
                            save_dream_entry(
                                &deps.storage,
                                &session.0,
                                &cycle_id,
                                source_sessions,
                                None,
                                &agent_config,
                                dream_start,
                                DreamStage::Consolidation,
                                &response,
                            )
                            .await;

                            response
                        }
                    }
                }
                _ => {
                    agent
                        .chat(request, session.clone(), vec![], vec![], Some(hooks), None)
                        .await?
                }
            };

            if let Some(ref tx) = response_tx {
                let _ = tx.send_async(res).await;
            }
        }
        VizierRequestContent::Command(cmd) => {
            tracing::warn!("unhandled command: {}", cmd);
        }
        VizierRequestContent::Reaction(event) => {
            tracing::info!(
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

async fn save_dream_entry(
    storage: &Arc<VizierStorage>,
    agent_id: &str,
    cycle_id: &str,
    source_sessions: Vec<String>,
    session_context: Option<String>,
    agent_config: &AgentConfig,
    start_time: chrono::DateTime<Utc>,
    stage: DreamStage,
    response: &VizierResponse,
) {
    let content = match &response.content {
        VizierResponseContent::Message { content, .. } => content.clone(),
        _ => return,
    };

    if content.is_empty() {
        return;
    }

    let now = Utc::now();
    let duration_ms = (now - start_time).num_milliseconds().max(0) as u64;

    let entry = DreamJournalEntry {
        id: uuid::Uuid::new_v4().to_string(),
        dream_cycle_id: cycle_id.to_string(),
        agent_id: agent_id.to_string(),
        timestamp: now,
        stage,
        source_sessions,
        session_context,
        content,
        duration_ms: Some(duration_ms),
        provider_used: Some(format!("{:?}", agent_config.provider)),
        model_used: Some(agent_config.model.clone()),
    };

    if let Err(e) = storage.save_dream_entry(entry).await {
        tracing::error!(
            "Failed to save dream journal entry for '{}': {}",
            agent_id,
            e
        );
    }
}
