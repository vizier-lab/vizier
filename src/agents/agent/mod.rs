use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;

use anyhow::Result;
use chrono::{DateTime, Utc};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use rig_core::{
    OneOrMany,
    completion::ToolDefinition,
    message::{AssistantContent, Message, UserContent},
};
use serde::{Deserialize, Serialize};
use tokio::time::{Instant, timeout};

use crate::{
    VizierError,
    agents::{
        agent::{
            model::{VizierModel, VizierModelTrait},
            system_prompt::{boot::boot_md, init_workspace, user::owner_md},
        },
        hook::{VizierSessionHook, VizierSessionHooks},
        skill::VizierSkills,
        tools::{ToolContext, VizierTools},
    },
    config::{VizierConfig, provider::ProviderVariant},
    dependencies::VizierDependencies,
    image_generation::VizierImageGen,
    indexer::VizierIndexer,
    schema::{
        AgentConfig, ErrorKind, Memory, SessionHistory, SessionHistoryContent, VizierAttachment,
        VizierAttachmentContent, VizierRequest, VizierRequestContent, VizierResponse,
        VizierResponseContent, VizierResponseStats, VizierSession,
        history_entries_to_messages, messages_to_history_entries,
    },
    storage::{
        VizierStorage,
        history::HistoryStorage,
        session_file::SessionFileStorage,
        user::{UserProfile, UserStorage},
    },
    stt::VizierStt,
    transport::VizierTransport,
    tts::VizierTts,
    utils::{agent_workspace, build_path, get_mime_type},
};

pub mod model;
pub mod system_prompt;

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct Subtask {
    title: String,
    prompt: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SubtaskArgs {
    #[schemars(description = "list of subtasks to be execute in paralel")]
    tasks: Vec<Subtask>,
}

#[derive(Clone)]
pub struct VizierAgent {
    pub workspace: String,
    pub global_workspace: String,
    pub storage: Arc<VizierStorage>,
    pub transport: VizierTransport,
    pub indexer: Option<crate::indexer::VizierIndexer>,

    model: VizierModel,
    tools: VizierTools,
    skills: VizierSkills,
    config: AgentConfig,
    owner_profile: Option<UserProfile>,
    stt: Option<Arc<VizierStt>>,
    tts: Option<Arc<VizierTts>>,
    image_gen: Option<Arc<VizierImageGen>>,
}

impl VizierAgent {
    pub async fn new(
        agent_id: String,
        deps: &VizierDependencies,
        agent_config: &AgentConfig,
        indexer: Option<crate::indexer::VizierIndexer>,
    ) -> Result<VizierAgent> {
        let workspace = agent_workspace(&deps.config.workspace, &agent_id)
            .to_string_lossy()
            .to_string();

        // Create STT instance if enabled (shared between auto-transcription and stt_transcribe tool)
        let stt = if agent_config.tools.stt.enabled {
            match VizierStt::new(&agent_config.tools.stt.settings, &deps.storage, &workspace).await
            {
                Ok(instance) => Some(Arc::new(instance)),
                Err(e) => {
                    tracing::error!("failed to create STT for agent {}: {}", agent_id, e);
                    None
                }
            }
        } else {
            None
        };

        // Create TTS instance if enabled (shared between audio reply and tts_generate tool)
        let tts = if agent_config.tools.tts.enabled {
            match VizierTts::new(&agent_config.tools.tts.settings, &deps.storage, &workspace).await
            {
                Ok(instance) => Some(Arc::new(instance)),
                Err(e) => {
                    tracing::error!("failed to create TTS for agent {}: {}", agent_id, e);
                    None
                }
            }
        } else {
            None
        };

        // Create image generation instance if enabled (used by image_generate tool)
        let image_gen = if agent_config.tools.image_gen.enabled {
            match VizierImageGen::new(&agent_config.tools.image_gen.settings, &deps.storage).await {
                Ok(instance) => Some(Arc::new(instance)),
                Err(e) => {
                    tracing::error!(
                        "failed to create image generation for agent {}: {}",
                        agent_id,
                        e
                    );
                    None
                }
            }
        } else {
            None
        };

        let model = VizierModel::new(agent_id.clone(), deps.clone(), agent_config).await?;
        let tools = VizierTools::new(
            agent_id.clone(),
            deps.clone(),
            agent_config,
            indexer.clone(),
            stt.clone(),
            tts.clone(),
            image_gen.clone(),
        )
        .await?;
        let skills = VizierSkills::new(agent_id.clone(), deps.clone()).await?;

        init_workspace(workspace.clone());

        // Fetch owner profile if owner_id is set
        let owner_profile = if let Some(ref owner_id) = agent_config.owner_id {
            deps.storage
                .get_user_profile(owner_id)
                .await
                .unwrap_or(None)
        } else {
            None
        };

        Ok(Self {
            model,
            tools,
            skills,
            config: agent_config.clone(),
            owner_profile,
            stt,
            tts,
            image_gen,
            workspace,
            global_workspace: deps.config.workspace.clone(),
            storage: deps.storage.clone(),
            transport: deps.transport.clone(),
            indexer,
        })
    }

    pub fn stt(&self) -> Option<&Arc<VizierStt>> {
        self.stt.as_ref()
    }

    async fn maybe_audio_reply(
        &self,
        response: VizierResponse,
        req: &VizierRequest,
    ) -> VizierResponse {
        if req.expect_audio_reply != Some(true) {
            return response;
        }
        let Some(ref tts) = self.tts else {
            return response;
        };

        let (text, stats) = match &response.content {
            VizierResponseContent::Message { content, stats } if !content.is_empty() => {
                (content.clone(), stats.clone())
            }
            _ => return response,
        };

        let voice = self
            .config
            .tools
            .tts
            .settings
            .voice
            .clone()
            .unwrap_or_else(|| {
                self.config
                    .tools
                    .tts
                    .settings
                    .provider
                    .default_voice()
                    .into()
            });
        let speed = self.config.tools.tts.settings.speed.unwrap_or(1.0);

        match tts.generate_speech(&text, &voice, speed).await {
            Ok(audio_bytes) => {
                let filename = format!("audio_reply_{}.wav", uuid::Uuid::new_v4());
                match self
                    .transport
                    .send_file_upload(filename.clone(), audio_bytes)
                    .await
                {
                    Ok(file_record) => VizierResponse {
                        timestamp: response.timestamp,
                        content: VizierResponseContent::AudioReply(
                            VizierAttachment {
                                filename,
                                content: VizierAttachmentContent::Local(file_record.url),
                            },
                            Some(text),
                            stats,
                        ),
                        attachments: response.attachments,
                    },
                    Err(e) => {
                        tracing::error!("TTS audio reply upload failed: {}", e);
                        response
                    }
                }
            }
            Err(e) => {
                tracing::error!("TTS audio reply failed: {}", e);
                response
            }
        }
    }

    pub async fn prepare_system_prompts(&self) -> Vec<Message> {
        // init workspace just in case
        init_workspace(self.workspace.clone());

        let agent_md = read_md_file(self.workspace.clone(), "SOUL.md".into());
        let ident_md = read_md_file(self.workspace.clone(), "IDENTITY.md".into());
        let boot = boot_md(
            self.config.name.clone(),
            self.config
                .description
                .clone()
                .unwrap_or("a Digital Steward".into()),
        );

        let mut res = vec![
            Message::system(boot),
            Message::system(
                self.config
                    .system_prompt
                    .clone()
                    .unwrap_or("You are a helpful and capable assistant. Follow the operating doctrine in BOOT.md.".into()),
            ),
        ];

        // Add owner info if available
        if let Some(ref owner) = self.owner_profile {
            res.push(Message::system(owner_md(owner)));
        }

        res.push(Message::system(agent_md));
        res.push(Message::system(ident_md));

        // Inject Always skills into system prompt
        if let Ok(always_skills) = self.skills.get_always_skills().await {
            for skill_content in always_skills {
                res.push(Message::system(skill_content));
            }
        }

        for document in &self.config.documents {
            res.push(Message::system(document.clone()));
        }

        res
    }

    pub async fn chat(
        &self,
        req: VizierRequest,
        session: VizierSession,
        session_history: Vec<SessionHistory>,
        memory: Vec<Memory>,
        hooks: Option<Arc<VizierSessionHooks>>,
    ) -> Result<VizierResponse> {
        let mut tools = self.tools.tools().await?;
        tools.extend(self.skills.get_ondemand_skills().await?);

        // Auto-transcribe AudioChat/AudioPrompt in-place
        let mut req = req;
        if let Some(ref stt) = self.stt {
            let audio_att = match &req.content {
                VizierRequestContent::AudioChat(att, None) => Some((att.clone(), false)),
                VizierRequestContent::AudioPrompt(att, None) => Some((att.clone(), true)),
                _ => None,
            };
            if let Some((att, _is_prompt)) = audio_att {
                let audio_bytes = self
                    .transport
                    .send_file_resolve(att.clone())
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;

                let language = self.config.tools.stt.settings.language.as_deref();
                let text = match stt.transcribe(&audio_bytes, &att.filename, language).await {
                    Ok(t) if !t.is_empty() => t,
                    Ok(_) => "[Voice message]".to_string(),
                    Err(e) => {
                        tracing::error!("STT transcription failed: {}", e);
                        "[Voice message - transcription failed]".to_string()
                    }
                };
                req.content = match req.content {
                    VizierRequestContent::AudioChat(a, _) => {
                        VizierRequestContent::AudioChat(a, Some(text))
                    }
                    VizierRequestContent::AudioPrompt(a, _) => {
                        VizierRequestContent::AudioPrompt(a, Some(text))
                    }
                    other => other,
                };
            }
        }

        // Match contextual skills against the task
        let task_text = match &req.content {
            VizierRequestContent::Chat(text) => text.clone(),
            VizierRequestContent::Prompt(text) => text.clone(),
            VizierRequestContent::Task(text) => text.clone(),
            VizierRequestContent::Command(text) => text.clone(),
            VizierRequestContent::AudioChat(_, Some(text)) => text.clone(),
            VizierRequestContent::AudioPrompt(_, Some(text)) => text.clone(),
            VizierRequestContent::AudioChat(_, None) => String::new(),
            VizierRequestContent::AudioPrompt(_, None) => String::new(),
            _ => String::new(),
        };
        if !task_text.is_empty() {
            let contextual_skills = self.skills.get_contextual_skills(&task_text).await?;
            tools.extend(contextual_skills);
        }

        let mut rng = StdRng::seed_from_u64(Utc::now().timestamp() as u64);
        let initiative_factor = rng.random_range(0_f32..=1_f32);

        let mut history = self.prepare_system_prompts().await;

        if memory.len() > 0 {
            let summarize_memories = memory
                .iter()
                .map(|memory| {
                    let mut truncated_content = memory.content.clone();
                    truncated_content.truncate(200);

                    let attachments_line = if memory.attachments.is_empty() {
                        String::new()
                    } else {
                        let files = memory.attachments.iter().map(|a| a.filename.as_str()).collect::<Vec<_>>().join(", ");
                        format!("\n**Attachments:** {} — use `memory_detail` to access\n", files)
                    };

                    format!(
                        "## {}\nslug: **{}**\n{}...\n{}**use the slug for more detail of this memory**\n \n---",
                        memory.title, memory.slug, truncated_content, attachments_line,
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            history.push(Message::system(format!(
                "# Related Memories\n{}",
                summarize_memories
            )));
        }

        history.extend(history_entries_to_messages(&session_history));

        if let Some(hooks) = hooks.clone() {
            req = hooks.on_request(req).await?;
        }

        // Store attachments in SessionFiles via FileManager
        for attachment in &req.attachments {
            let content = self
                .transport
                .send_file_resolve(attachment.clone())
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            let file_record = self
                .transport
                .send_file_upload(attachment.filename.clone(), content)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            let mime_type = get_mime_type(&attachment.filename);
            self.storage
                .save_session_file(
                    &session,
                    &attachment.filename,
                    &mime_type,
                    file_record.size,
                    &file_record.id,
                )
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
        }
        // Clear attachments so they don't get injected directly into the conversation
        req.attachments.clear();

        if let VizierRequestContent::SilentRead(_) = req.content {
            if initiative_factor > self.config.silent_read_initiative_chance {
                return Ok(VizierResponse {
                    timestamp: chrono::Utc::now(),
                    content: VizierResponseContent::Empty,
                    attachments: vec![],
                });
            }
        }

        // Optimistically save request before prompt
        self.storage
            .save_session_history(
                session.clone(),
                SessionHistoryContent::Request(req.clone()),
            )
            .await?;

        let original_history_len = history.len();

        let prompt_result = self
            .prompt(
                req.to_message(&self.global_workspace)?,
                history,
                0,
                hooks.clone(),
                false,
                &ToolContext {
                    session: session.clone(),
                    pending_attachments: Arc::new(Mutex::new(vec![])),
                },
            )
            .await;

        let (output, stats, attachments, final_history) = match prompt_result {
            Ok(result) => result,
            Err((err, partial_history)) => {
                // Save tool call/result entries from partial history
                let new_messages = &partial_history[original_history_len..];
                let mut tool_entries = messages_to_history_entries(new_messages);
                tool_entries.retain(|e| !matches!(e, SessionHistoryContent::AssistantMessage(_)));
                for entry in tool_entries {
                    self.storage
                        .save_session_history(session.clone(), entry)
                        .await?;
                }

                // Determine error kind
                let err_str = err.to_string();
                let kind = ErrorKind::classify(&err_str);

                // Save error entry as Response with Error content
                self.storage
                    .save_session_history(
                        session.clone(),
                        SessionHistoryContent::Response(VizierResponse {
                            timestamp: chrono::Utc::now(),
                            content: VizierResponseContent::Error {
                                kind,
                                message: err_str,
                            },
                            attachments: vec![],
                        }),
                    )
                    .await?;

                return Err(err.into());
            }
        };

        // Save tool calls/results from the delta, skipping the last message
        // (final assistant response) which is saved explicitly below with full stats
        let new_messages = &final_history[original_history_len..];
        if new_messages.len() > 1 {
            let mut tool_entries =
                messages_to_history_entries(&new_messages[..new_messages.len() - 1]);
            tool_entries.retain(|e| !matches!(e, SessionHistoryContent::AssistantMessage(_)));
            for entry in tool_entries {
                self.storage
                    .save_session_history(session.clone(), entry)
                    .await?;
            }
        }

        // Save response with full stats and attachments
        let mut response = VizierResponse {
            timestamp: chrono::Utc::now(),
            content: VizierResponseContent::Message {
                content: output,
                stats: Some(stats),
            },
            attachments,
        };
        response = self.maybe_audio_reply(response, &req).await;

        self.storage
            .save_session_history(
                session.clone(),
                SessionHistoryContent::Response(response.clone()),
            )
            .await?;

        if let Some(hooks) = hooks.clone() {
            response = hooks.on_response(response).await?;
        }

        Ok(response)
    }

    pub async fn prompt(
        &self,
        message: Message,
        history: Vec<Message>,
        turn_depth: usize,
        hooks: Option<Arc<VizierSessionHooks>>,
        _is_subagent: bool,
        ctx: &ToolContext,
    ) -> std::result::Result<(String, VizierResponseStats, Vec<VizierAttachment>, Vec<Message>), (anyhow::Error, Vec<Message>)> {
        let mut history = history.clone();
        let mut turn_depth = turn_depth;
        let max_turn_depth = self.config.thinking_depth;
        let mut tools = self.tools.tools().await.map_err(|e| (e, history.clone()))?;
        tools.extend(self.skills.get_ondemand_skills().await.map_err(|e| (e, history.clone()))?);

        let output: String;

        let mut message = message;

        let start = Instant::now();
        let prompt_timeout = *self.config.prompt_timeout;

        let mut input_tokens: u64 = 0;
        let mut cached_input_tokens: u64 = 0;
        let mut total_cached_input_tokens: u64 = 0;
        let mut total_input_tokens: u64 = 0;
        let mut total_output_tokens: u64 = 0;
        let mut total_tokens: u64 = 0;
        let mut cache_creation_input_tokens: u64 = 0;
        let mut total_cache_creation_input_tokens: u64 = 0;
        let mut current_context_size: Option<u64> = None;

            loop {
                turn_depth += 1;
                if max_turn_depth > 0 && turn_depth > max_turn_depth {
                    return Err((anyhow::anyhow!(VizierError(format!(
                        "thinking depth exceeding {}",
                        max_turn_depth
                    ))), history));
                }

                // Check prompt timeout
                if start.elapsed() > prompt_timeout {
                    return Err((anyhow::anyhow!(VizierError(format!(
                        "prompt timed out after {:?}",
                        prompt_timeout
                    ))), history));
                }

                let (message_id, choices, usage) = self
                    .model
                    .completion(message.clone(), history.clone(), tools.clone())
                    .await
                    .map_err(|e| (e, history.clone()))?;

                history.push(message);

                history.push(Message::Assistant {
                    id: message_id.clone(),
                    content: choices.clone(),
                });

                if turn_depth == 1 {
                    input_tokens = usage.input_tokens;
                    cached_input_tokens = usage.cached_input_tokens;
                    cache_creation_input_tokens = usage.cache_creation_input_tokens;
                }

                total_input_tokens += usage.input_tokens;
                total_cached_input_tokens += usage.cached_input_tokens;
                total_cache_creation_input_tokens += usage.cache_creation_input_tokens;
                total_output_tokens += usage.output_tokens;
                total_tokens += usage.total_tokens;
                current_context_size = Some(usage.input_tokens);

                let (tool_calls, others): (Vec<_>, Vec<_>) = choices
                    .iter()
                    .partition(|choice| matches!(choice, AssistantContent::ToolCall(_)));

                if tool_calls.is_empty() {
                    output = others
                        .iter()
                        .filter_map(|item| {
                            if let AssistantContent::Text(text) = item {
                                Some(text.to_string())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    break;
                }

                let mut tool_responses = vec![];
                let mut pending_images: Vec<UserContent> = vec![];
                for call in tool_calls.iter().filter_map(|item| {
                    if let AssistantContent::ToolCall(call) = item {
                        Some(call)
                    } else {
                        None
                    }
                }) {
                    let (mut function_name, mut args) = (
                        call.function.name.clone(),
                        serde_json::to_string(&call.function.arguments).unwrap(),
                    );
                    if let Some(hooks) = hooks.clone() {
                        (function_name, args) = hooks.on_tool_call(function_name, args).await.map_err(|e| (e, history.clone()))?;
                    }

                    // handle custom skill
                    let mut tool_res = if function_name.clone().starts_with("SKILL__") {
                        let output = self.call_skill(function_name.clone()).await;
                        VizierResponse {
                            timestamp: Utc::now(),
                            content: VizierResponseContent::ToolResponse {
                                response: serde_json::Value::String(output),
                            },
                            attachments: vec![],
                        }
                    } else {
                        let tool_server = self.tools.clone();
                        let ctx_clone = ctx.clone();
                        match timeout(*self.config.tools.timeout, async {
                            tool_server
                                .call(function_name.clone(), args, &ctx_clone)
                                .await
                        })
                        .await
                        {
                            Err(_elapsed) => {
                                // Tool timeout - return error with partial history
                                return Err((anyhow::anyhow!(VizierError(format!(
                                    "Tool '{}' timed out after {:?}",
                                    function_name,
                                    *self.config.tools.timeout
                                ))), history));
                            }
                            Ok(Err(err)) => VizierResponse {
                                timestamp: Utc::now(),
                                content: VizierResponseContent::ToolResponse {
                                    response: serde_json::Value::String(err.to_string()),
                                },
                                attachments: vec![],
                            },
                            Ok(Ok(s)) => s,
                        }
                    };

                    if let Some(hooks) = hooks.clone() {
                        tool_res = hooks.on_tool_response(tool_res).await.map_err(|e| (e, history.clone()))?;
                    }

                    // Store tool attachments in SessionFiles (except from read_image_file)
                    if !tool_res.attachments.is_empty() && function_name != "read_image_file" {
                        let mut stored_files = vec![];
                        for attachment in &tool_res.attachments {
                            if let Ok(content) =
                                self.transport.send_file_resolve(attachment.clone()).await
                            {
                                if let Ok(file_record) = self
                                    .transport
                                    .send_file_upload(attachment.filename.clone(), content)
                                    .await
                                {
                                    let mime_type = get_mime_type(&attachment.filename);
                                    if self
                                        .storage
                                        .save_session_file(
                                            &ctx.session,
                                            &attachment.filename,
                                            &mime_type,
                                            file_record.size,
                                            &file_record.id,
                                        )
                                        .await
                                        .is_ok()
                                    {
                                        stored_files.push(attachment.filename.clone());
                                    }
                                }
                            }
                        }
                        tool_res.attachments.clear();
                        if !stored_files.is_empty() {
                            if let VizierResponseContent::ToolResponse { response } =
                                &mut tool_res.content
                            {
                                let files = stored_files.join(", ");
                                let notification = format!("\n\n[+{} to session files]", files);
                                *response = serde_json::Value::String(format!(
                                    "{}{}",
                                    response.as_str().unwrap_or(""),
                                    notification
                                ));
                            }
                        }
                    }

                    // For read_image_file with images: collect for separate user messages
                    // (most providers drop images from tool results)
                    if function_name == "read_image_file" && !tool_res.attachments.is_empty() {
                        let image_attachments: Vec<_> = tool_res.attachments.drain(..).collect();
                        tool_responses.push(tool_res.to_tool_response_content(
                            call.id.clone(),
                            call.call_id.clone(),
                            &self.global_workspace,
                        ).map_err(|e| (e, history.clone()))?);
                        for attachment in &image_attachments {
                            pending_images
                                .push(attachment.to_user_content(&self.global_workspace).map_err(|e| (e, history.clone()))?);
                        }
                    } else {
                        tool_responses.push(tool_res.to_tool_response_content(
                            call.id.clone(),
                            call.call_id.clone(),
                            &self.global_workspace,
                        ).map_err(|e| (e, history.clone()))?);
                    }
                }

                // Push tool response message to history
                let tool_message = Message::User {
                    content: OneOrMany::many(tool_responses).unwrap(),
                };

                if pending_images.is_empty() {
                    message = tool_message
                } else {
                    history.push(tool_message);

                    // Push each image as a separate user message
                    for img_content in pending_images {
                        history.push(Message::User {
                            content: OneOrMany::one(img_content),
                        });
                    }

                    // Continue loop with next model call
                    message = Message::User {
                        content: OneOrMany::one(rig_core::message::UserContent::Text(
                            "[Images loaded into context. Continue processing.]".into(),
                        )),
                    };
                }
            }

            let attachments = ctx.pending_attachments.lock().await.drain(..).collect();

            Ok((
                output,
                VizierResponseStats {
                    total_tokens,
                    total_cached_input_tokens,
                    total_input_tokens,
                    total_output_tokens,
                    input_tokens,
                    cached_input_tokens,
                    duration: start.elapsed(),
                    cache_creation_input_tokens,
                    total_cache_creation_input_tokens,
                    current_context_size,
                    context_window: self.model.context_window(),
                },
                attachments,
                history,
            ))
    }

    pub async fn call_skill(&self, skill_name: String) -> String {
        let slug = skill_name.replace("SKILL__", "");
        match self.skills.get_skill_content(slug).await {
            Err(err) => err.to_string(),
            Ok(content) => content.unwrap_or("".into()),
        }
    }

    pub async fn dream_chat(
        &self,
        req: VizierRequest,
        session: VizierSession,
        session_history: Vec<SessionHistory>,
        hooks: Option<Arc<VizierSessionHooks>>,
        deps: &VizierDependencies,
    ) -> Result<VizierResponse> {
        // Use dream model if both provider and model are configured
        let dream_model = if let (Some(provider), Some(model)) = (
            self.config.dream_provider.as_ref(),
            self.config.dream_model.as_ref(),
        ) {
            VizierModel::new_with_override(
                deps,
                &self.config,
                Some((provider.clone(), model.clone())),
            )
            .await?
        } else {
            self.model.clone()
        };

        // Build dream tools
        let tools = self
            .tools
            .dream_tools(self.config.name.clone(), deps.storage.clone())
            .await?;

        // Prepare system prompts (same as chat)
        let mut history = self.prepare_system_prompts().await;

        // Extend with session history
        history.extend(history_entries_to_messages(&session_history));

        let mut req = req;
        if let Some(hooks) = hooks.clone() {
            req = hooks.on_request(req).await?;
        }

        let (output, stats) = self
            .dream_prompt(
                &dream_model,
                req.to_message(&self.global_workspace)?,
                history,
                tools,
                hooks.clone(),
                deps,
                &ToolContext {
                    session,
                    pending_attachments: Arc::new(Mutex::new(vec![])),
                },
            )
            .await?;

        let mut response = VizierResponse {
            timestamp: chrono::Utc::now(),
            content: VizierResponseContent::Message {
                content: output,
                stats: Some(stats),
            },
            attachments: vec![],
        };
        response = self.maybe_audio_reply(response, &req).await;
        if let Some(hooks) = hooks.clone() {
            response = hooks.on_response(response).await?;
        }

        Ok(response)
    }

    async fn dream_prompt(
        &self,
        model: &VizierModel,
        message: Message,
        history: Vec<Message>,
        tools: Vec<ToolDefinition>,
        hooks: Option<Arc<VizierSessionHooks>>,
        deps: &VizierDependencies,
        ctx: &ToolContext,
    ) -> Result<(String, VizierResponseStats)> {
        timeout(*self.config.prompt_timeout, async {
            let mut history = history.clone();
            let mut turn_depth = 0;
            let max_turn_depth = self.config.thinking_depth;
            let tools = tools.clone();

            let output: String;
            let mut message = message;
            let start = Instant::now();

            let mut input_tokens: u64 = 0;
            let mut cached_input_tokens: u64 = 0;
            let mut total_cached_input_tokens: u64 = 0;
            let mut total_input_tokens: u64 = 0;
            let mut total_output_tokens: u64 = 0;
            let mut total_tokens: u64 = 0;
            let mut cache_creation_input_tokens: u64 = 0;
            let mut total_cache_creation_input_tokens: u64 = 0;
            let mut current_context_size: Option<u64> = None;

            loop {
                turn_depth += 1;
                if max_turn_depth > 0 && turn_depth > max_turn_depth {
                    return Err(anyhow::anyhow!(VizierError(format!(
                        "thinking depth exceeding {}",
                        max_turn_depth
                    ))));
                }

                let (message_id, choices, usage) = model
                    .completion(message.clone(), history.clone(), tools.clone())
                    .await?;

                history.push(message);

                history.push(Message::Assistant {
                    id: message_id.clone(),
                    content: choices.clone(),
                });

                if turn_depth == 1 {
                    input_tokens = usage.input_tokens;
                    cached_input_tokens = usage.cached_input_tokens;
                    cache_creation_input_tokens = usage.cache_creation_input_tokens;
                }

                total_input_tokens += usage.input_tokens;
                total_cached_input_tokens += usage.cached_input_tokens;
                total_cache_creation_input_tokens += usage.cache_creation_input_tokens;
                total_output_tokens += usage.output_tokens;
                total_tokens += usage.total_tokens;
                current_context_size = Some(usage.input_tokens);

                let (tool_calls, others): (Vec<_>, Vec<_>) = choices
                    .iter()
                    .partition(|choice| matches!(choice, AssistantContent::ToolCall(_)));

                if tool_calls.is_empty() {
                    output = others
                        .iter()
                        .filter_map(|item| {
                            if let AssistantContent::Text(text) = item {
                                Some(text.to_string())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    break;
                }

                let mut tool_responses = vec![];
                let mut pending_attachments: Vec<UserContent> = vec![];
                for call in tool_calls.iter().filter_map(|item| {
                    if let AssistantContent::ToolCall(call) = item {
                        Some(call)
                    } else {
                        None
                    }
                }) {
                    let (mut function_name, mut args) = (
                        call.function.name.clone(),
                        serde_json::to_string(&call.function.arguments).unwrap(),
                    );
                    if let Some(hooks) = hooks.clone() {
                        (function_name, args) = hooks.on_tool_call(function_name, args).await?;
                    }

                    // handle custom skill
                    let mut tool_res = if function_name.clone().starts_with("SKILL__") {
                        let output = self.call_skill(function_name.clone()).await;
                        VizierResponse {
                            timestamp: Utc::now(),
                            content: VizierResponseContent::ToolResponse {
                                response: serde_json::Value::String(output),
                            },
                            attachments: vec![],
                        }
                    } else {
                        // Use dream_call for dream tool dispatch
                        let ctx_clone = ctx.clone();
                        match timeout(*self.config.tools.timeout, async {
                            self.tools
                                .dream_call(
                                    function_name.clone(),
                                    args,
                                    &self.config.name,
                                    &deps.storage,
                                    &ctx_clone,
                                )
                                .await
                        })
                        .await?
                        {
                            Err(err) => VizierResponse {
                                timestamp: Utc::now(),
                                content: VizierResponseContent::ToolResponse {
                                    response: serde_json::Value::String(err.to_string()),
                                },
                                attachments: vec![],
                            },
                            Ok(s) => s,
                        }
                    };

                    if let Some(hooks) = hooks.clone() {
                        tool_res = hooks.on_tool_response(tool_res).await?;
                    }

                    // Store tool attachments in SessionFiles (except from read_image_file)
                    if !tool_res.attachments.is_empty() && function_name != "read_image_file" {
                        let mut stored_files = vec![];
                        for attachment in &tool_res.attachments {
                            if let Ok(content) =
                                self.transport.send_file_resolve(attachment.clone()).await
                            {
                                if let Ok(file_record) = self
                                    .transport
                                    .send_file_upload(attachment.filename.clone(), content)
                                    .await
                                {
                                    let mime_type = get_mime_type(&attachment.filename);
                                    if self
                                        .storage
                                        .save_session_file(
                                            &ctx.session,
                                            &attachment.filename,
                                            &mime_type,
                                            file_record.size,
                                            &file_record.id,
                                        )
                                        .await
                                        .is_ok()
                                    {
                                        stored_files.push(attachment.filename.clone());
                                    }
                                }
                            }
                        }
                        tool_res.attachments.clear();
                        if !stored_files.is_empty() {
                            if let VizierResponseContent::ToolResponse { response } =
                                &mut tool_res.content
                            {
                                let files = stored_files.join(", ");
                                let notification = format!("\n\n[+{} to session files]", files);
                                *response = serde_json::Value::String(format!(
                                    "{}{}",
                                    response.as_str().unwrap_or(""),
                                    notification
                                ));
                            }
                        }
                    }

                    // For read_image_file with images: collect for separate user messages
                    // (most providers drop images from tool results)
                    if function_name == "read_image_file" && !tool_res.attachments.is_empty() {
                        let image_attachments: Vec<_> = tool_res.attachments.drain(..).collect();
                        tool_responses.push(tool_res.to_tool_response_content(
                            call.id.clone(),
                            call.call_id.clone(),
                            &self.global_workspace,
                        )?);
                        for attachment in &image_attachments {
                            pending_attachments
                                .push(attachment.to_user_content(&self.global_workspace)?);
                        }
                    } else {
                        tool_responses.push(tool_res.to_tool_response_content(
                            call.id.clone(),
                            call.call_id.clone(),
                            &self.global_workspace,
                        )?);
                    }
                }

                let tool_message = Message::User {
                    content: OneOrMany::many(tool_responses).unwrap(),
                };

                if pending_attachments.is_empty() {
                    message = tool_message;
                } else {
                    history.push(tool_message);

                    // Push each image as a separate user message
                    for img_content in pending_attachments {
                        history.push(Message::User {
                            content: OneOrMany::one(img_content),
                        });
                    }

                    // Continue loop with next model call
                    message = Message::User {
                        content: OneOrMany::one(rig_core::message::UserContent::Text(
                            "[Attachment(s) loaded into context. Continue processing.]".into(),
                        )),
                    };
                }
            }

            Ok((
                output,
                VizierResponseStats {
                    total_tokens,
                    total_cached_input_tokens,
                    total_input_tokens,
                    total_output_tokens,
                    input_tokens,
                    cached_input_tokens,
                    duration: start.elapsed(),
                    cache_creation_input_tokens,
                    total_cache_creation_input_tokens,
                    current_context_size,
                    context_window: model.context_window(),
                },
            ))
        })
        .await?
    }
}

pub fn read_md_file(workspace: String, file: String) -> String {
    let path = build_path(&workspace, &[&file]);

    fs::read_to_string(path).unwrap()
}
