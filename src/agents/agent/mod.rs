use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::Mutex;

use anyhow::Result;
use chrono::{DateTime, Utc};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};
use tokio::time::{Instant, timeout};
use tracing::Instrument;

use crate::{
    VizierError,
    agents::{
        agent::{
            model::{VizierModel, usage::UsageScope},
            system_prompt::{
                boot::boot_md,
                context::{context_md, with_context},
                reactions::reaction_digest,
                init_workspace,
                sandbox::sandbox_md,
                scheduled_run::scheduled_run_md,
                user::owner_md,
            },
        },
        hook::{VizierSessionHook, VizierSessionHooks},
        skill::VizierSkills,
        tools::{
            BaseTools, CategorySummary, ToolContext, VizierTools, compose, traced_tool_call,
            turn_categories,
        },
    },
    config::{VizierConfig, provider::ProviderId},
    llm::types::{AssistantPart, CompletionRequest, Message, Part, ToolChoice, ToolDefinition},
    dependencies::VizierDependencies,
    image_generation::VizierImageGen,
    indexer::VizierIndexer,
    integrations::{IntegrationTools, agent_status, kind::AgentToolContext},
    schema::{
        AgentConfig, AgentId, ErrorKind, IntegrationStatus, MemoryPassageResult, SessionHistory, SessionHistoryContent, Skill,
        VizierAttachment, VizierAttachmentContent, VizierRequest, VizierRequestContent,
        VizierResponse, VizierResponseContent, VizierResponseStats, VizierSession,
        history_entries_to_messages, message_for_model, messages_for_model,
        messages_to_history_entries, seal_tool_calls,
    },
    skill::invocation::Invoked,
    storage::{
        VizierStorage,
        agent::AgentStorage,
        history::HistoryStorage,
        integration::IntegrationStorage,
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

    agent_id: AgentId,
    deps: VizierDependencies,
    model: VizierModel,
    /// Built once per instance; integration changes never rebuild it.
    base: Arc<BaseTools>,
    /// The composed tools. A turn loads one snapshot at its start and keeps it to the end
    /// (contracts/runtime.md G2); `refresh_integrations` swaps in a new one for the next turn.
    tools: Arc<ArcSwap<VizierTools>>,
    /// Serialises refreshes, so the last one to finish read the latest storage.
    refresh_lock: Arc<Mutex<()>>,
    skills: VizierSkills,
    config: AgentConfig,
    core: String,
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
            match VizierStt::new(&agent_config.tools.stt.settings, &deps, &workspace, &agent_id).await
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
            match VizierTts::new(
                &agent_config.tools.tts.settings,
                &deps,
                &deps.config.workspace,
                &agent_id,
            )
            .await
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
            match VizierImageGen::new(&agent_config.tools.image_gen.settings, &deps).await {
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
        let base = Arc::new(
            BaseTools::new(
                agent_id.clone(),
                deps.clone(),
                agent_config,
                indexer.clone(),
                stt.clone(),
                tts.clone(),
                image_gen.clone(),
            )
            .await?,
        );
        let integrations = Self::build_integrations(&agent_id, deps, None).await;
        let tools = Arc::new(ArcSwap::from_pointee(compose(&base, Arc::new(integrations))));
        let skills = VizierSkills::new(agent_id.clone(), deps.clone(), indexer.clone()).await?;

        // Best-effort re-embed of every skill so vector-based recommendation stays fresh
        // across restarts; individual create/update/delete tool calls keep it fresh in between.
        tokio::spawn({
            let skills = skills.clone();
            async move {
                if let Err(e) = skills.reindex_all().await {
                    tracing::warn!("skill reindex failed: {}", e);
                }
            }
        });

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

        let core = deps
            .storage
            .get_agent_core(&agent_id)
            .await
            .unwrap_or(None)
            .unwrap_or_else(|| crate::constant::CORE_MD.to_string());

        Ok(Self {
            agent_id,
            deps: deps.clone(),
            model,
            base,
            tools,
            refresh_lock: Arc::new(Mutex::new(())),
            skills,
            config: agent_config.clone(),
            core,
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

    /// The integration layer from the agent's attachments in storage, reusing what `previous`
    /// already built for integrations that didn't change. A storage error yields no
    /// integrations rather than failing the agent.
    async fn build_integrations(
        agent_id: &AgentId,
        deps: &VizierDependencies,
        previous: Option<&IntegrationTools>,
    ) -> IntegrationTools {
        let integrations = match deps.storage.list_agent_integrations(agent_id).await {
            Ok(integrations) => integrations,
            Err(e) => {
                tracing::warn!(agent_id, "cannot read the agent's integrations: {e}");
                vec![]
            }
        };
        let mut layer = IntegrationTools::build(
            &deps.integration_kinds,
            &integrations,
            &tool_context(agent_id, deps),
            previous,
        );
        layer.self_management = match self_management_tools(agent_id, deps).await {
            Ok(tools) => tools,
            Err(e) => {
                tracing::warn!(agent_id, "cannot read the agent's self-management grants: {e}");
                Default::default()
            }
        };
        layer
    }

    /// Re-read the agent's integrations and swap in tools composed from them, for the next
    /// turn to pick up; turns already running keep their snapshot (contracts/runtime.md G1–G3).
    /// A refresh that fails keeps the previous tools.
    pub async fn refresh_integrations(&self) {
        let _guard = self.refresh_lock.lock().await;
        let integrations = match self.deps.storage.list_agent_integrations(&self.agent_id).await {
            Ok(integrations) => integrations,
            Err(e) => {
                tracing::warn!(
                    agent_id = %self.agent_id,
                    "integration refresh failed, keeping the current tools: {e}"
                );
                return;
            }
        };
        let self_management = match self_management_tools(&self.agent_id, &self.deps).await {
            Ok(tools) => tools,
            Err(e) => {
                tracing::warn!(
                    agent_id = %self.agent_id,
                    "grant refresh failed, keeping the current tools: {e}"
                );
                return;
            }
        };
        let current = self.tools.load_full();
        let mut layer = IntegrationTools::build(
            &self.deps.integration_kinds,
            &integrations,
            &tool_context(&self.agent_id, &self.deps),
            Some(&current.integrations),
        );
        layer.self_management = self_management;
        self.tools.store(Arc::new(compose(&self.base, Arc::new(layer))));
        tracing::debug!(agent_id = %self.agent_id, "integration tools refreshed");
    }

    /// This agent's view of each integration attached to it (research R9).
    pub async fn integration_status(&self) -> HashMap<String, IntegrationStatus> {
        let snapshot = self.tools.load_full();
        let integrations = self
            .deps
            .storage
            .list_agent_integrations(&self.agent_id)
            .await
            .unwrap_or_default();
        integrations
            .iter()
            .filter_map(|integration| {
                let kind = self.deps.integration_kinds.get(&integration.kind)?;
                Some((
                    integration.id.clone(),
                    agent_status(
                        kind.as_ref(),
                        integration,
                        &snapshot.integrations,
                        &self.deps.integration_health,
                    ),
                ))
            })
            .collect()
    }

    /// What this agent's model resolved to.
    pub fn model_info(&self) -> &model::ModelInfo {
        self.model.info()
    }

    /// Settings that work but likely aren't what was meant, for agent health.
    pub fn health_warnings(&self) -> Vec<String> {
        let mut warnings = vec![];
        let read_image = &self.config.tools.read_image;
        let explicit_vision = read_image.settings.provider.is_some()
            && read_image.settings.model.as_deref().is_some_and(|m| !m.trim().is_empty());
        if read_image.enabled && !explicit_vision && self.model.info().caps.image_input() == Some(false) {
            warnings.push("read_image: main model cannot view images; configure a vision model".into());
        }
        warnings
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
                        ..Default::default()
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

    /// Takes the session because one section is conditional on it: the scheduled-run
    /// framing. The question is answered through `VizierSession::is_scheduled_task()` and
    /// nowhere else — selecting on the request content kind would catch the dream cycle,
    /// which sends `Unattended` too and carries its own framing.
    ///
    /// `categories` are the turn's: the snapshot's own, followed by any the person's client
    /// declared for this turn (`tools::turn_categories`).
    pub async fn prepare_system_prompts(
        &self,
        categories: &[CategorySummary],
        session: &VizierSession,
    ) -> Vec<Message> {
        init_workspace(self.workspace.clone());

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

        res.insert(
            1,
            Message::system(sandbox_md(
                categories,
                &self.config.tools.timeout.to_string(),
            )),
        );

        // Add owner info if available
        if let Some(ref owner) = self.owner_profile {
            res.push(Message::system(owner_md(owner)));
        }

        res.push(Message::system(self.core.clone()));

        for document in &self.config.documents {
            res.push(Message::system(document.clone()));
        }

        // Appended last, after CORE and documents, so the cacheable boot/CORE prefix stays
        // byte-identical to an interactive turn's.
        if session.is_scheduled_task() {
            res.push(Message::system(scheduled_run_md()));
        }

        res
    }

    pub async fn chat(
        &self,
        req: VizierRequest,
        session: VizierSession,
        session_history: Vec<SessionHistory>,
        memory: Vec<MemoryPassageResult>,
        skills: Vec<Skill>,
        invoked: Option<Invoked>,
        hooks: Option<Arc<VizierSessionHooks>>,
        checkpoint_handover: Option<String>,
    ) -> Result<VizierResponse> {
        let span = self.invoke_agent_span(&req, &session, false);
        self.chat_turn(req, session, session_history, memory, skills, invoked, hooks, checkpoint_handover)
            .instrument(span)
            .await
    }

    /// The `invoke_agent` span for one turn: a child of whatever sent the request (a tool
    /// call, a background job), or a root for a person's message or a scheduled run.
    fn invoke_agent_span(&self, req: &VizierRequest, session: &VizierSession, dream: bool) -> tracing::Span {
        let span = tracing::info_span!(
            "invoke_agent",
            otel.name = %format!("invoke_agent {}", self.agent_id),
            gen_ai.operation.name = "invoke_agent",
            gen_ai.agent.id = %self.agent_id,
            gen_ai.conversation.id = %session.to_slug(),
            vizier.channel.type = tracing::field::Empty,
            vizier.channel.id = tracing::field::Empty,
            vizier.topic = session.2.as_deref(),
            vizier.http.user = tracing::field::Empty,
            vizier.task.slug = tracing::field::Empty,
            vizier.inter_agent.participants = tracing::field::Empty,
            vizier.dream.stage = tracing::field::Empty,
            vizier.dream.source = tracing::field::Empty,
            vizier.request.kind = req.content.kind(),
            vizier.background_depth = req.background_depth,
            vizier.queue.wait_ms = req.trace.queue_wait_ms(),
            vizier.dream = dream,
        );
        record_channel(&span, &session.1);
        req.trace.adopt(&span);
        span
    }

    async fn chat_turn(
        &self,
        req: VizierRequest,
        session: VizierSession,
        session_history: Vec<SessionHistory>,
        memory: Vec<MemoryPassageResult>,
        skills: Vec<Skill>,
        invoked: Option<Invoked>,
        hooks: Option<Arc<VizierSessionHooks>>,
        checkpoint_handover: Option<String>,
    ) -> Result<VizierResponse> {
        let background_depth = req.background_depth;
        // Auto-transcribe AudioChat/AudioPrompt in-place
        let mut req = req;
        if let Some(ref stt) = self.stt {
            let audio_att = match &req.content {
                VizierRequestContent::AudioChat(att, None) => Some((att.clone(), false)),
                VizierRequestContent::AudioPrompt(att, None) => Some((att.clone(), true)),
                _ => None,
            };
            if let Some((att, _is_prompt)) = audio_att {
                let language = self.config.tools.stt.settings.language.as_deref();
                let text = match self.transport.send_file_resolve(att.clone()).await {
                    Err(e) => {
                        tracing::error!("failed to load voice message for transcription: {}", e);
                        "[Voice message - could not be loaded]".to_string()
                    }
                    Ok(audio_bytes) => {
                        match stt.transcribe(&audio_bytes, &att.filename, language).await {
                            Ok(t) if !t.is_empty() => t,
                            Ok(_) => "[Voice message]".to_string(),
                            Err(e) => {
                                tracing::error!("STT transcription failed: {}", e);
                                "[Voice message - transcription failed]".to_string()
                            }
                        }
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

        let mut rng = StdRng::seed_from_u64(Utc::now().timestamp() as u64);
        let initiative_factor = rng.random_range(0_f32..=1_f32);

        // The one snapshot this turn uses, system prompt and every tool call alike (G2).
        let tools = self.tools.load_full();

        // The tools the person's client declared for this turn, if its connection is still
        // open now that the turn has started (specs/019-client-tools R4). They are named
        // against this snapshot's catalogue, and the client is told the names before the
        // model runs. Taken off the request so nothing downstream holds the connection.
        let client = match req.client.take() {
            Some(binding) if binding.connection.is_open() => {
                let catalogue = tools.router.catalogue().await;
                let client = crate::agents::client::ClientTools::resolve(
                    &binding,
                    &catalogue,
                    &tools.router.client_reserved_categories(),
                    session.clone(),
                    self.storage.clone(),
                    self.transport.clone(),
                );
                client.announce();
                Some(client)
            }
            _ => None,
        };
        let ctx = ToolContext {
            loaded_images: Default::default(),
            session: session.clone(),
            pending_attachments: Arc::new(Mutex::new(vec![])),
            hooks: hooks.clone(),
            background_depth,
            client,
        };

        let mut history = self
            .prepare_system_prompts(&turn_categories(&tools.categories, &ctx), &session)
            .await;

        // Inject checkpoint handover as system context
        if let Some(handover) = checkpoint_handover {
            history.push(Message::system(format!(
                "# Conversation Context (Previous Checkpoint)\n\
                 The following is a summary of the conversation before this session was checkpointed. \
                 Use this to maintain context and continuity.\n\n{}",
                handover
            )));
        }

        history.extend(history_entries_to_messages(&session_history));
        // 1: this turn's own request follows the loaded history.
        let digest = reaction_digest(&session_history, 1);

        if let Some(hooks) = hooks.clone() {
            req = hooks.on_request(req).await?;
        }

        // Store attachments in SessionFiles via FileManager. One that fails is left out and
        // the model told so, rather than failing the turn over one file (#47).
        let mut unavailable = vec![];
        for attachment in &req.attachments {
            if let Err(e) = self.store_session_file(&session, attachment).await {
                tracing::warn!(
                    "attachment '{}' could not be stored, continuing without it: {}",
                    attachment.filename,
                    e
                );
                unavailable.push(attachment.filename.clone());
            }
        }
        // Clear attachments so they don't get injected directly into the conversation
        req.attachments.clear();

        // Always save request to history (even for SilentRead that may not get a response)
        self.save_history_or_log(&session, SessionHistoryContent::Request(req.clone()))
            .await;

        if let VizierRequestContent::SilentRead(_) = req.content {
            if initiative_factor > self.config.silent_read_initiative_chance {
                return Ok(VizierResponse {
                    timestamp: chrono::Utc::now(),
                    content: VizierResponseContent::Empty,
                    attachments: vec![],
                    ..Default::default()
                });
            }
        }

        let original_history_len = history.len();

        // An invoked skill: history keeps what the person typed (`/release-notes v1.2`), the
        // model is sent only the input, with the skill itself in the context block (spec 021).
        let mut message = match &invoked {
            Some(invoked) => {
                let mut model_req = req.clone();
                if let VizierRequestContent::Chat(text) = &mut model_req.content {
                    *text = invoked.message().to_string();
                }
                model_req.to_message(&self.global_workspace)?
            }
            None => req.to_message(&self.global_workspace)?,
        };
        if !unavailable.is_empty() {
            message = with_note(
                message,
                format!(
                    "[These attachments could not be loaded: {}]",
                    unavailable.join(", ")
                ),
            );
        }

        let prompt_result = self
            .prompt_with(
                &tools,
                with_context(message, context_md(
                        &memory,
                        &skills,
                        invoked.as_ref().map(|invoked| &invoked.skill),
                        digest.as_deref(),
                    )),
                history,
                0,
                hooks.clone(),
                false,
                &ctx,
            )
            .await;

        let (output, stats, attachments, final_history) = match prompt_result {
            Ok(result) => result,
            Err((err, partial_history)) => {
                // Save narration and tool call/result entries from partial history.
                // `prompt` returns its full record, which always starts with `history`, so
                // the slice holds; `get` keeps a regression from panicking the turn.
                let new_messages = partial_history
                    .get(original_history_len..)
                    .unwrap_or_default();
                let tool_entries = messages_to_history_entries(new_messages);
                for entry in tool_entries {
                    self.save_history_or_log(&session, entry).await;
                }

                // Determine error kind
                let err_str = err.to_string();
                let kind = ErrorKind::classify(&err_str);

                // Save error entry as Response with Error content. A failed save is logged,
                // never reported in place of the error that ended the turn (#47).
                self.save_history_or_log(
                    &session,
                    SessionHistoryContent::Response(VizierResponse {
                        timestamp: chrono::Utc::now(),
                        content: VizierResponseContent::Error {
                            kind,
                            message: err_str,
                        },
                        attachments: vec![],
                        ..Default::default()
                    }),
                )
                .await;

                return Err(err.into());
            }
        };

        // Save tool calls/results from the delta, skipping the last message
        // (final assistant response) which is saved explicitly below with full stats
        let new_messages = &final_history[original_history_len..];
        if new_messages.len() > 1 {
            // `AssistantMessage` entries are kept: the narration an agent writes alongside
            // its tool calls is part of what it did on the way to the answer, and it is
            // recorded ahead of those tool calls so replay can merge the two back into the
            // single assistant message the model actually sent.
            let tool_entries =
                messages_to_history_entries(&new_messages[..new_messages.len() - 1]);
            for entry in tool_entries {
                self.save_history_or_log(&session, entry).await;
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
            ..Default::default()
        };
        response = self.maybe_audio_reply(response, &req).await;

        // The answer exists and is paid for: a failed save loses it from history, not from
        // the person waiting for it (#47).
        let uid = self
            .save_history_or_log(&session, SessionHistoryContent::Response(response.clone()))
            .await;
        // After the save, so the uid is never part of the stored entry: it travels on the
        // response so every channel can address the reply it is about to render.
        response.history_uid = uid;

        if let Some(hooks) = hooks.clone() {
            response = hooks.on_response(response).await?;
        }

        Ok(response)
    }

    /// Run one prompt on the agent's current tools.
    pub async fn prompt(
        &self,
        message: Message,
        history: Vec<Message>,
        turn_depth: usize,
        hooks: Option<Arc<VizierSessionHooks>>,
        is_subagent: bool,
        ctx: &ToolContext,
    ) -> std::result::Result<
        (
            String,
            VizierResponseStats,
            Vec<VizierAttachment>,
            Vec<Message>,
        ),
        (anyhow::Error, Vec<Message>),
    > {
        let tools = self.tools.load_full();
        self.prompt_with(&tools, message, history, turn_depth, hooks, is_subagent, ctx)
            .await
    }

    async fn prompt_with(
        &self,
        snapshot: &Arc<VizierTools>,
        message: Message,
        history: Vec<Message>,
        turn_depth: usize,
        hooks: Option<Arc<VizierSessionHooks>>,
        _is_subagent: bool,
        ctx: &ToolContext,
    ) -> std::result::Result<
        (
            String,
            VizierResponseStats,
            Vec<VizierAttachment>,
            Vec<Message>,
        ),
        (anyhow::Error, Vec<Message>),
    > {
        // Every return but the answered one drops this, marking the turn's answered rounds as
        // spent on a failed turn (spec 018 FR-004); an aborted task drops it too.
        let guard = self.model.guard();
        let mut history = history.clone();
        let mut full_history = history.clone();
        let mut turn_depth = turn_depth;
        let max_turn_depth = self.config.thinking_depth;
        let tools = snapshot
            .tools()
            .await
            .map_err(|e| (e, full_history.clone()))?;

        let output: String;

        let mut message = message;
        // The turn's own request, which a mid-turn checkpoint keeps ahead of the exchange in
        // progress.
        let request = message.clone();

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

        let mut depth_exhausted = false;

        loop {
            turn_depth += 1;
            // Past the last tool-using step, one more completion answers with what the turn
            // found, instead of the turn failing and losing it (#48).
            let wrapping_up = max_turn_depth > 0 && turn_depth > max_turn_depth;
            if wrapping_up {
                message = with_wrap_up_note(message, max_turn_depth);
            }

            // Check prompt timeout
            if start.elapsed() > prompt_timeout {
                return Err(fail_turn(
                    anyhow::anyhow!(VizierError(format!(
                        "prompt timed out after {:?}",
                        prompt_timeout
                    ))),
                    full_history,
                    Some(message),
                    vec![],
                ));
            }

            // The model gets the narrowed view; `full_history` keeps the full record,
            // which is what storage and the WebUI are served from.
            let response = self
                .model
                .complete_until(
                    completion_request(&history, &message, &tools, wrapping_up),
                    Some(start + prompt_timeout),
                    &UsageScope::turn(Some(ctx.session.clone()), guard.turn(), turn_depth as u32),
                )
                .await
                .map_err(|e| fail_turn(e, full_history.clone(), Some(message.clone()), vec![]))?;
            let (choices, usage) = (response.content, response.usage);
            let choices = if wrapping_up {
                answer_without_tool_calls(choices)
            } else {
                choices
            };

            tracing::debug!(
                turn_depth = turn_depth,
                input_tokens = usage.input_tokens,
                output_tokens = usage.output_tokens,
                cached_input_tokens = usage.cached_input_tokens,
                "LLM completion received"
            );

            history.push(message.clone());
            full_history.push(message.clone());

            history.push(Message::Assistant(choices.clone()));
            full_history.push(Message::Assistant(choices.clone()));

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
                .partition(|choice| matches!(choice, AssistantPart::ToolCall { .. }));

            if tool_calls.is_empty() {
                output = others
                    .iter()
                    .filter_map(|item| {
                        if let AssistantPart::Text(text) = item {
                            Some(text.to_string())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                depth_exhausted = wrapping_up;
                if wrapping_up {
                    tracing::info!(
                        thinking_depth = max_turn_depth,
                        "tool budget used up; answered with what the turn had"
                    );
                }

                // A final answer leaves no tool call waiting, so the checkpoint is safe here,
                // and it's what keeps the session's next turn small.
                self.maybe_checkpoint(snapshot, current_context_size, &mut history, &request, None, &hooks, ctx)
                    .await;
                break;
            }

            let mut tool_responses: Vec<Message> = vec![];
            let mut pending_images: Vec<Part> = vec![];
            for (call_id, call_name, call_arguments) in tool_calls.iter().filter_map(|item| {
                if let AssistantPart::ToolCall { id, name, arguments, .. } = item {
                    Some((id, name, arguments))
                } else {
                    None
                }
            }) {
                let args = serde_json::to_string(call_arguments).map_err(|e| {
                    fail_turn(e.into(), full_history.clone(), None, tool_responses.clone())
                })?;
                let (mut function_name, mut args) = (call_name.clone(), args);
                if let Some(hooks) = hooks.clone() {
                    (function_name, args) = hooks
                        .on_tool_call(function_name, args)
                        .await
                        .map_err(|e| {
                            fail_turn(e, full_history.clone(), None, tool_responses.clone())
                        })?;
                }

                let tool_server = snapshot.clone();
                let ctx_clone = ctx.clone();
                let traced_args = args.clone();
                let mut tool_res = match traced_tool_call(&function_name, call_id, &traced_args, *self.config.tools.timeout, async {
                    tool_server
                        .call(function_name.clone(), args, &ctx_clone)
                        .await
                })
                .await
                {
                    // Answered like a failed call, so the model can work around it; the turn
                    // stays bounded by `prompt_timeout` and `thinking_depth` (#45).
                    Err(_elapsed) => tool_timeout_response(&function_name, *self.config.tools.timeout),
                    Ok(Err(err)) => VizierResponse {
                        timestamp: Utc::now(),
                        content: VizierResponseContent::ToolResponse {
                            response: serde_json::Value::String(err.to_string()),
                        },
                        attachments: vec![],
                        ..Default::default()
                    },
                    Ok(Ok(s)) => s,
                };

                if let Some(hooks) = hooks.clone() {
                    tool_res = hooks
                        .on_tool_response(tool_res)
                        .await
                        .map_err(|e| {
                            fail_turn(e, full_history.clone(), None, tool_responses.clone())
                        })?;
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
                            note_stored_files(response, &stored_files);
                        }
                    }
                }

                let response_content = tool_res
                    .to_tool_result(call_id.clone(), &self.global_workspace)
                    .map_err(|e| {
                        fail_turn(e, full_history.clone(), None, tool_responses.clone())
                    })?;
                tool_responses.push(response_content);
            }

            // Images the round's tools loaded go in separate user messages, since most
            // providers drop images from tool results.
            for attachment in ctx.loaded_images.lock().await.drain(..) {
                pending_images.push(attachment.to_part(&self.global_workspace).map_err(|e| {
                    fail_turn(e, full_history.clone(), None, tool_responses.clone())
                })?);
            }

            if tool_responses.is_empty() {
                return Err(fail_turn(
                    anyhow::anyhow!("tool calls produced no responses"),
                    full_history,
                    None,
                    vec![],
                ));
            }

            if pending_images.is_empty() {
                // Every result but the last joins the history now; the last is the message
                // the next completion carries.
                message = tool_responses.pop().expect("checked non-empty");
                history.extend(tool_responses.iter().cloned());
                full_history.extend(tool_responses);
            } else {
                history.extend(tool_responses.iter().cloned());
                full_history.extend(tool_responses);

                // Each image as a separate user message: most providers drop images from
                // tool results.
                for img in pending_images {
                    let img_msg = Message::User(vec![img]);
                    history.push(img_msg.clone());
                    full_history.push(img_msg);
                }

                // Continue loop with next model call
                message = Message::User(vec![Part::Text(
                    "[Images loaded into context. Continue processing.]".into(),
                )]);
            }

            // Only now, with every tool call of the reply answered: checkpointing between the
            // completion and its tool results dropped the calls the results answer (#63).
            self.maybe_checkpoint(
                snapshot,
                current_context_size,
                &mut history,
                &request,
                Some(&message),
                &hooks,
                ctx,
            )
            .await;
        }

        let attachments = ctx.pending_attachments.lock().await.drain(..).collect();

        guard.succeeded();
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
                depth_exhausted,
                provider: Some(self.model.info().provider.clone()),
                model: Some(self.model.info().model.clone()),
            },
            attachments,
            full_history,
        ))
    }

    /// Compact the turn's context once the last completion used `checkpoint_threshold` of the
    /// window: generate a handover, save it as the session's checkpoint, and rebuild `history`
    /// as system prompts, handover, the turn's `request` and the exchange in progress (from the
    /// last assistant message on). `next` is the message about to be sent, if any; the handover
    /// covers it too.
    ///
    /// Called only where no tool call is waiting for its result, so the rebuilt history never
    /// starts a tool exchange halfway (#63). If the handover can't be generated the turn carries
    /// on with its full context and tries again after the next completion (#47).
    async fn maybe_checkpoint(
        &self,
        tools: &VizierTools,
        context_size: Option<u64>,
        history: &mut Vec<Message>,
        request: &Message,
        next: Option<&Message>,
        hooks: &Option<Arc<VizierSessionHooks>>,
        ctx: &ToolContext,
    ) {
        let (Some(ctx_size), Some(ctx_window)) = (context_size, self.model.context_window())
        else {
            return;
        };
        let usage_ratio = ctx_size as f64 / ctx_window as f64;
        if usage_ratio < self.config.checkpoint_threshold {
            return;
        }
        tracing::info!(
            "Context window usage at {:.1}% ({} / {} tokens), creating checkpoint",
            usage_ratio * 100.0,
            ctx_size,
            ctx_window
        );

        // No digest: the turn's request already carries it in its context block.
        let mut summarised = history.clone();
        summarised.extend(next.cloned());
        let handover = match self.generate_handover_message(&summarised, None, ctx).await {
            Ok(handover) => handover,
            Err(e) => {
                tracing::warn!("checkpoint handover failed, continuing without compacting: {}", e);
                return;
            }
        };

        if let Err(e) = self
            .save_checkpoint(ctx.session.clone(), handover.clone())
            .await
        {
            tracing::error!("Failed to save checkpoint: {}", e);
        }

        if let Some(hooks) = hooks
            && let Err(e) = hooks.on_handover(handover.clone()).await
        {
            tracing::error!("Failed to notify hooks about handover: {}", e);
        }

        let in_progress = history
            .iter()
            .rposition(|m| matches!(m, Message::Assistant(_)))
            .map(|at| history.split_off(at))
            .unwrap_or_default();

        history.clear();
        history.extend(
            self.prepare_system_prompts(&turn_categories(&tools.categories, ctx), &ctx.session)
                .await,
        );
        if let Some(ref msg) = handover {
            history.push(Message::system(format!(
                "# Conversation Context (Previous Checkpoint)\n\
                 The following is a summary of the conversation before this session was checkpointed.\n\n{}",
                msg
            )));
        }
        history.push(request.clone());
        history.extend(in_progress);
    }

    /// Store one attachment of the request as a session file.
    async fn store_session_file(
        &self,
        session: &VizierSession,
        attachment: &VizierAttachment,
    ) -> Result<()> {
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
                session,
                &attachment.filename,
                &mime_type,
                file_record.size,
                &file_record.id,
            )
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }

    /// Save a history entry without letting a failure end the turn (#47): retried briefly
    /// while sqlite is busy, then logged. The entry's uid when it was saved.
    async fn save_history_or_log(
        &self,
        session: &VizierSession,
        content: SessionHistoryContent,
    ) -> Option<String> {
        match crate::storage::history::save_with_busy_retry(&self.storage, session, content).await {
            Ok(uid) => Some(uid),
            Err(e) => {
                tracing::error!(
                    agent_id = %session.0,
                    "failed to save session history, the turn continues without it: {e}"
                );
                None
            }
        }
    }

    async fn save_checkpoint(
        &self,
        session: VizierSession,
        handover: Option<String>,
    ) -> Result<()> {
        self.storage.save_checkpoint(session, handover).await?;
        Ok(())
    }

    /// `reactions` is the digest list for the history being summarised, so feedback survives
    /// the checkpoint and reaches the dream cycle, which extracts from the handover.
    pub async fn generate_handover_message(
        &self,
        history: &[Message],
        reactions: Option<&str>,
        ctx: &ToolContext,
    ) -> Result<Option<String>> {
        let scope = UsageScope::handover(Some(ctx.session.clone()));
        generate_handover_with_model(&self.model, history, reactions, &scope).await
    }

    pub fn skills(&self) -> &VizierSkills {
        &self.skills
    }

    pub async fn recommend_skills(&self, query: &str) -> Result<Vec<Skill>> {
        self.skills.recommend_skills(query, 5, 0.4).await
    }

    pub async fn dream_chat(
        &self,
        req: VizierRequest,
        session: VizierSession,
        session_history: Vec<SessionHistory>,
        hooks: Option<Arc<VizierSessionHooks>>,
        deps: &VizierDependencies,
    ) -> Result<VizierResponse> {
        let span = self.invoke_agent_span(&req, &session, true);
        self.dream_turn(req, session, session_history, hooks, deps)
            .instrument(span)
            .await
    }

    async fn dream_turn(
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
                &self.agent_id,
                deps,
                &self.config,
                Some((provider.clone(), model.clone())),
            )
            .await?
        } else {
            self.model.clone()
        };

        let snapshot = self.tools.load_full();
        // Build dream tools
        let tools = snapshot
            .dream_tools(self.config.name.clone(), deps.storage.clone())
            .await?;

        // Prepare system prompts (same as chat)
        let mut history = self.prepare_system_prompts(&snapshot.categories, &session).await;

        // Extend with session history
        history.extend(history_entries_to_messages(&session_history));

        let mut req = req;
        if let Some(hooks) = hooks.clone() {
            req = hooks.on_request(req).await?;
        }

        let (output, stats) = self
            .dream_prompt(
                &snapshot,
                &dream_model,
                with_context(
                    req.to_message(&self.global_workspace)?,
                    context_md(&[], &[], None, reaction_digest(&session_history, 1).as_deref()),
                ),
                history,
                tools,
                hooks.clone(),
                deps,
                &ToolContext {
                    loaded_images: Default::default(),
                    client: None,
                    session,
                    pending_attachments: Arc::new(Mutex::new(vec![])),
                    hooks: hooks.clone(),
                    background_depth: 0,
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
            ..Default::default()
        };
        response = self.maybe_audio_reply(response, &req).await;
        if let Some(hooks) = hooks.clone() {
            response = hooks.on_response(response).await?;
        }

        Ok(response)
    }

    async fn dream_prompt(
        &self,
        snapshot: &VizierTools,
        model: &VizierModel,
        message: Message,
        history: Vec<Message>,
        tools: Vec<ToolDefinition>,
        hooks: Option<Arc<VizierSessionHooks>>,
        deps: &VizierDependencies,
        ctx: &ToolContext,
    ) -> Result<(String, VizierResponseStats)> {
        // As in `prompt_with`: a dream that errors or times out leaves its rounds failed.
        let guard = model.guard();
        let turn = guard.turn().clone();
        let outcome = timeout(*self.config.prompt_timeout, async {
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

            let mut depth_exhausted = false;

            loop {
                turn_depth += 1;
                // As in `prompt`: the dream answers with what it has rather than failing (#48).
                let wrapping_up = max_turn_depth > 0 && turn_depth > max_turn_depth;
                if wrapping_up {
                    message = with_wrap_up_note(message, max_turn_depth);
                }

                let response = model
                    .complete(
                        completion_request(&history, &message, &tools, wrapping_up),
                        &UsageScope::turn(Some(ctx.session.clone()), &turn, turn_depth as u32),
                    )
                    .await?;
                let usage = response.usage;
                let choices = if wrapping_up {
                    answer_without_tool_calls(response.content)
                } else {
                    response.content
                };

                history.push(message);

                history.push(Message::Assistant(choices.clone()));

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
                    .partition(|choice| matches!(choice, AssistantPart::ToolCall { .. }));

                if tool_calls.is_empty() {
                    output = others
                        .iter()
                        .filter_map(|item| {
                            if let AssistantPart::Text(text) = item {
                                Some(text.to_string())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    depth_exhausted = wrapping_up;

                    break;
                }

                let mut tool_responses: Vec<Message> = vec![];
                let mut pending_attachments: Vec<Part> = vec![];
                for (call_id, call_name, call_arguments) in tool_calls.iter().filter_map(|item| {
                    if let AssistantPart::ToolCall { id, name, arguments, .. } = item {
                        Some((id, name, arguments))
                    } else {
                        None
                    }
                }) {
                    let (mut function_name, mut args) =
                        (call_name.clone(), serde_json::to_string(call_arguments)?);
                    if let Some(hooks) = hooks.clone() {
                        (function_name, args) = hooks.on_tool_call(function_name, args).await?;
                    }

                    // Use dream_call for dream tool dispatch
                    let ctx_clone = ctx.clone();
                    let traced_args = args.clone();
                    let mut tool_res = match traced_tool_call(&function_name, call_id, &traced_args, *self.config.tools.timeout, async {
                        snapshot
                            .dream_call(
                                function_name.clone(),
                                args,
                                &self.config.name,
                                &deps.storage,
                                &ctx_clone,
                            )
                            .await
                    })
                    .await
                    {
                        Err(_elapsed) => {
                            tool_timeout_response(&function_name, *self.config.tools.timeout)
                        }
                        Ok(Err(err)) => VizierResponse {
                            timestamp: Utc::now(),
                            content: VizierResponseContent::ToolResponse {
                                response: serde_json::Value::String(err.to_string()),
                            },
                            attachments: vec![],
                            ..Default::default()
                        },
                        Ok(Ok(s)) => s,
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
                                note_stored_files(response, &stored_files);
                            }
                        }
                    }

                    tool_responses
                        .push(tool_res.to_tool_result(call_id.clone(), &self.global_workspace)?);
                }

                for attachment in ctx.loaded_images.lock().await.drain(..) {
                    pending_attachments.push(attachment.to_part(&self.global_workspace)?);
                }

                if tool_responses.is_empty() {
                    anyhow::bail!("tool calls produced no responses");
                }

                if pending_attachments.is_empty() {
                    message = tool_responses.pop().expect("checked non-empty");
                    history.extend(tool_responses);
                } else {
                    history.extend(tool_responses);

                    // Push each image as a separate user message
                    for img in pending_attachments {
                        history.push(Message::User(vec![img]));
                    }

                    // Continue loop with next model call
                    message = Message::User(vec![Part::Text(
                        "[Attachment(s) loaded into context. Continue processing.]".into(),
                    )]);
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
                    depth_exhausted,
                    provider: Some(model.info().provider.clone()),
                    model: Some(model.info().model.clone()),
                },
            ))
        })
        .await?;
        if outcome.is_ok() {
            guard.succeeded();
        }
        outcome
    }
}

/// The error a failed turn returns, with every record it has to keep.
///
/// `unsent` is the message the next model call would have carried — after a round of tool
/// calls, their results — which only reaches `full_history` once that call is made, so a
/// failure before it would otherwise lose them. Any tool call still unanswered after
/// `completed` is answered with the error, so the stored turn never ends on a tool call
/// with no result (issue #58).
/// `note` appended to a user message, after what the person sent.
fn with_note(message: Message, note: String) -> Message {
    match message {
        Message::User(mut content) => {
            content.push(Part::Text(note));
            Message::User(content)
        }
        other => other,
    }
}

/// One completion: the history and the message about to be sent, as the model sees them. A
/// wrap-up asks for text only (`tool_choice: none`); tools stay defined, since providers reject
/// a history holding tool calls when no tools are.
fn completion_request(
    history: &[Message],
    message: &Message,
    tools: &[ToolDefinition],
    answer_only: bool,
) -> CompletionRequest {
    let mut messages = messages_for_model(history);
    messages.push(message_for_model(message));
    CompletionRequest {
        messages,
        tools: tools.to_vec(),
        tool_choice: answer_only.then_some(ToolChoice::None),
        max_tokens: None,
    }
}

/// The result a timed-out tool call hands the model. The call is abandoned, but whatever it
/// did before the deadline may already have happened.
/// Said when the wrap-up completion offered nothing to say.
const DEPTH_EXHAUSTED_FALLBACK: &str =
    "I ran out of steps for this turn before I could finish. Ask me to continue and I'll pick \
     up from here.";

/// `message` (normally the latest tool results) with a note that the turn's tool budget is
/// spent, so the wrap-up completion answers instead of reaching for another tool (#48).
fn with_wrap_up_note(message: Message, thinking_depth: usize) -> Message {
    let note = format!(
        "[You have used all {thinking_depth} tool steps for this turn, and no more tool calls \
         will run. Answer now with what you have. Say plainly what is still unfinished or \
         unverified, so the user can ask you to continue.]"
    );
    match message {
        Message::User(mut content) => {
            content.push(Part::Text(note));
            Message::User(content)
        }
        Message::ToolResult { call_id, mut content } => {
            content.push(Part::Text(note));
            Message::ToolResult { call_id, content }
        }
        other => other,
    }
}

/// The wrap-up completion as the turn keeps it: its text (and reasoning), never a tool call.
/// A call would never get a result, and a call without one breaks the next request on
/// providers that check the pairing (#63). Some providers ignore `tool_choice: none`, so a
/// reply may be nothing but calls; it then says the turn ran out of steps.
fn answer_without_tool_calls(choices: Vec<AssistantPart>) -> Vec<AssistantPart> {
    let mut kept: Vec<AssistantPart> = choices
        .into_iter()
        .filter(|choice| !matches!(choice, AssistantPart::ToolCall { .. }))
        .collect();
    let has_text = kept
        .iter()
        .any(|choice| matches!(choice, AssistantPart::Text(text) if !text.trim().is_empty()));
    if !has_text {
        kept.push(AssistantPart::Text(DEPTH_EXHAUSTED_FALLBACK.into()));
    }
    kept
}

fn tool_timeout_response(function_name: &str, limit: std::time::Duration) -> VizierResponse {
    VizierResponse {
        timestamp: Utc::now(),
        content: VizierResponseContent::ToolResponse {
            response: serde_json::Value::String(format!(
                "error: tool '{function_name}' timed out after {limit:?} and was abandoned; \
                 any effect it had before then may still have happened"
            )),
        },
        attachments: vec![],
        ..Default::default()
    }
}

fn fail_turn(
    err: anyhow::Error,
    mut full_history: Vec<Message>,
    unsent: Option<Message>,
    completed: Vec<Message>,
) -> (anyhow::Error, Vec<Message>) {
    full_history.extend(unsent);
    seal_tool_calls(&mut full_history, completed, &err.to_string());
    (err, full_history)
}

pub fn read_md_file(workspace: String, file: String) -> String {
    let path = build_path(&workspace, &[&file]);

    fs::read_to_string(path).unwrap()
}

/// The agent's self-management tools for the families it is granted now (none without a grant).
async fn self_management_tools(
    agent_id: &AgentId,
    deps: &VizierDependencies,
) -> anyhow::Result<crate::agents::tools::VizierToolSet> {
    let granted: Vec<_> = deps
        .storage
        .list_grants(agent_id)
        .await?
        .into_iter()
        .map(|grant| grant.family)
        .collect();
    Ok(crate::agents::tools::self_integrations::toolset(agent_id, deps, &granted))
}

fn tool_context(agent_id: &AgentId, deps: &VizierDependencies) -> AgentToolContext {
    AgentToolContext {
        agent_id: agent_id.clone(),
        storage: deps.storage.clone(),
        health: deps.integration_health.clone(),
    }
}

pub async fn generate_handover_with_model(
    model: &VizierModel,
    history: &[Message],
    reactions: Option<&str>,
    scope: &UsageScope,
) -> Result<Option<String>> {
    let mut summary_history = messages_for_model(history);

    if let Some(list) = reactions {
        summary_history.push(Message::user_text(format!(
            "Reactions people gave to your messages in this conversation (feedback, not messages):\n{list}"
        ))?);
    }

    summary_history.push(Message::user_text(
        "Analyze this conversation and extract key context for continuation. Include:\n\
         1. **Key Facts**: Important information, data, or discoveries\n\
         2. **Decisions Made**: Any choices, directions, or conclusions reached\n\
         3. **Current State**: What is being worked on right now\n\
         4. **Pending Tasks**: Incomplete work or next steps\n\
         5. **Constraints**: Any limitations, preferences, or requirements\n\
         6. **Feedback**: Reactions people gave to your messages and what they suggest about \
         what worked and what did not. Omit this item if there were no reactions.\n\n\
         Format as a concise structured summary. Be factual and precise.",
    )?);
    summary_history.push(Message::user_text("Extract conversation context for checkpoint.")?);

    let response = model
        .complete(
            CompletionRequest {
                messages: summary_history,
                ..Default::default()
            },
            scope,
        )
        .await?;

    let output = response
        .content
        .iter()
        .filter_map(|item| {
            if let AssistantPart::Text(text) = item {
                Some(text.to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    if output.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(output))
    }
}

/// Tell the model which tool attachments were saved to session files. Text
/// responses get a trailing note; structured ones (e.g. a Python execution
/// report) keep their content and gain a `session_files_added` field.
fn note_stored_files(response: &mut serde_json::Value, stored_files: &[String]) {
    match response {
        serde_json::Value::Object(map) => {
            map.insert("session_files_added".into(), serde_json::json!(stored_files));
        }
        other => {
            let notification = format!("\n\n[+{} to session files]", stored_files.join(", "));
            *other = serde_json::Value::String(format!(
                "{}{}",
                other.as_str().unwrap_or(""),
                notification
            ));
        }
    }
}

#[cfg(test)]
mod wrap_up_tests {
    use super::*;

    fn call() -> AssistantPart {
        AssistantPart::ToolCall {
            id: "call-1".into(),
            name: "shell_exec".into(),
            arguments: serde_json::json!({"commands": "ls"}),
            provider_data: None,
        }
    }

    fn texts(choices: &[AssistantPart]) -> Vec<String> {
        choices
            .iter()
            .filter_map(|c| match c {
                AssistantPart::Text(t) => Some(t.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_note_follows_the_tool_results_in_the_same_message() {
        let results = Message::tool_result("call-1", "done");
        let Message::ToolResult { call_id, content } = with_wrap_up_note(results, 3) else {
            panic!("still a tool result");
        };
        assert_eq!(call_id, "call-1");
        assert_eq!(content.len(), 2);
        assert_eq!(content[0], Part::Text("done".into()), "results stay first");
        assert!(content[1].text().unwrap().contains("all 3 tool steps"));
    }

    /// #48: a provider that ignores `tool_choice: none` may still call a tool; the call is
    /// dropped (it would never get a result, #63) and the text kept.
    #[test]
    fn a_wrap_up_keeps_its_text_and_drops_tool_calls() {
        let kept = answer_without_tool_calls(vec![AssistantPart::Text("Here's what I found.".into()), call()]);
        assert!(!kept.iter().any(|c| matches!(c, AssistantPart::ToolCall { .. })));
        assert_eq!(texts(&kept), vec!["Here's what I found."]);
    }

    #[test]
    fn a_wrap_up_with_nothing_to_say_says_it_ran_out_of_steps() {
        let kept = answer_without_tool_calls(vec![call()]);
        assert_eq!(texts(&kept), vec![DEPTH_EXHAUSTED_FALLBACK]);

        let blank = answer_without_tool_calls(vec![AssistantPart::Text("  ".into())]);
        assert!(texts(&blank).contains(&DEPTH_EXHAUSTED_FALLBACK.to_string()));
    }

    #[test]
    fn a_wrap_up_request_asks_for_text_only_and_keeps_the_tools() {
        let tool = ToolDefinition {
            name: "t".into(),
            description: "d".into(),
            parameters: serde_json::json!({}),
        };
        let history = vec![Message::system("s")];
        let message = Message::user_text("hi").unwrap();
        let req = completion_request(&history, &message, std::slice::from_ref(&tool), true);
        assert_eq!(req.tool_choice, Some(ToolChoice::None));
        assert_eq!(req.tools, vec![tool.clone()]);
        assert_eq!(req.messages.len(), 2);
        assert_eq!(completion_request(&history, &message, &[tool], false).tool_choice, None);
    }
}

/// `gen_ai.conversation.id` taken apart, so a trace can be found by any piece of it (the
/// Discord channel, the HTTP user, the task) without matching the whole slug.
fn record_channel(span: &tracing::Span, channel: &crate::schema::VizierChannelId) {
    use crate::schema::{DreamStage, VizierChannelId};

    let kind = match channel {
        VizierChannelId::DiscordChanel(id) => {
            span.record("vizier.channel.id", id.to_string());
            "discord"
        }
        VizierChannelId::TelegramChannel(id) => {
            span.record("vizier.channel.id", id.to_string());
            "telegram"
        }
        VizierChannelId::HTTP(user, id) => {
            span.record("vizier.http.user", user.as_str());
            span.record("vizier.channel.id", id.as_str());
            "http"
        }
        VizierChannelId::Task(slug, _) => {
            span.record("vizier.task.slug", slug.as_str());
            "task"
        }
        VizierChannelId::InterAgent(participants) => {
            span.record("vizier.inter_agent.participants", participants.join(","));
            "inter_agent"
        }
        VizierChannelId::System => "system",
        VizierChannelId::Subagent => "subagent",
        VizierChannelId::Dream(source, stage) => {
            span.record(
                "vizier.dream.stage",
                match stage {
                    DreamStage::Extraction => "extraction",
                    DreamStage::Consolidation => "consolidation",
                },
            );
            span.record("vizier.dream.source", source.to_slug());
            "dream"
        }
    };
    span.record("vizier.channel.type", kind);
}
