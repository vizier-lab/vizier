use std::{fs, sync::Arc};

use anyhow::Result;
use base64::Engine;
use chrono::Utc;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use rig::{
    OneOrMany,
    message::{AssistantContent, ImageMediaType, Message, ToolResultContent, UserContent},
};
use rustpython_vm::common::str::ascii::IntoAsciiString;
use serde::{Deserialize, Serialize};
use tokio::time::{Instant, timeout};

use crate::{
    VizierError,
    agents::{
        agent::{
            model::{VizierModel, VizierModelTrait},
            system_prompt::{boot::boot_md, init_workspace, user::primary_user_md},
        },
        hook::{VizierSessionHook, VizierSessionHooks},
        skill::VizierSkills,
        tools::VizierTools,
    },
    config::{agent::AgentConfig, user::UserConfig},
    dependencies::VizierDependencies,
    schema::{
        Memory, SessionHistory, SessionHistoryContent, VizierAttachment, VizierAttachmentContent,
        VizierRequest, VizierRequestContent, VizierResponse, VizierResponseContent,
        VizierResponseStats,
    },
    utils::{agent_workspace, build_path, get_mime_type},
};

mod model;
mod system_prompt;

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

    model: VizierModel,
    tools: VizierTools,
    skills: VizierSkills,
    config: AgentConfig,
    primary_user: UserConfig,
}

impl VizierAgent {
    pub async fn new(agent_id: String, deps: &VizierDependencies) -> Result<VizierAgent> {
        let agent_config = deps.config.agents.get(&agent_id.clone()).unwrap();

        let model = VizierModel::new(agent_id.clone(), deps.clone()).await?;
        let tools = VizierTools::new(agent_id.clone(), deps.clone()).await?;
        let skills = VizierSkills::new(agent_id.clone(), deps.clone()).await?;

        let workspace = agent_workspace(&deps.config.workspace, &agent_id)
            .to_string_lossy()
            .to_string();
        init_workspace(workspace.clone());

        Ok(Self {
            model,
            tools,
            skills,
            config: agent_config.clone(),
            primary_user: deps.config.primary_user.clone(),
            workspace,
        })
    }

    pub async fn prepare_system_prompts(&self) -> Vec<Message> {
        // init workspace just in case
        init_workspace(self.workspace.clone());

        let agent_md = read_md_file(self.workspace.clone(), "AGENT.md".into());
        let ident_md = read_md_file(self.workspace.clone(), "IDENTITY.md".into());
        let boot = boot_md();

        let mut res = vec![
            Message::system(boot),
            Message::system(
                self.config
                    .system_prompt
                    .clone()
                    .unwrap_or("you are a helpful assistant".into()),
            ),
            Message::system(primary_user_md(&self.primary_user)),
            Message::system(agent_md),
            Message::system(ident_md),
        ];

        for document in &self.config.documents {
            res.push(Message::system(document.clone()));
        }

        res
    }

    pub async fn chat(
        &self,
        req: VizierRequest,
        session_history: Vec<SessionHistory>,
        memory: Vec<Memory>,
        hooks: Option<Arc<VizierSessionHooks>>,
    ) -> Result<VizierResponse> {
        let mut tools = self.tools.tools().await?;
        tools.extend(self.skills.get_skills().await?);

        let mut rng = StdRng::seed_from_u64(Utc::now().timestamp() as u64);
        let initiative_factor = rng.random_range(0_f32..=1_f32);

        let mut history = self.prepare_system_prompts().await;

        if memory.len() > 0 {
            let summarize_memories = memory
                .iter()
                .map(|memory| {
                    let mut truncated_content = memory.content.clone();
                    truncated_content.truncate(200);

                    format!(
                        "## {}\nslug: **{}**\n{}...\n**use the slug for more detail of this memory**\n \n---",
                        memory.title, memory.slug, truncated_content,
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            history.push(Message::system(format!(
                "# Related Memories\n{}",
                summarize_memories
            )));
        }

        history.extend(
            session_history
                .iter()
                .map(|history| match &history.content {
                    SessionHistoryContent::Request(req) => Message::user(req.to_prompt().unwrap()),
                    SessionHistoryContent::Response(r) => {
                        if let VizierResponseContent::Message { content, .. } = &r.content {
                            Message::assistant(content.clone())
                        } else {
                            Message::assistant("".to_string())
                        }
                    }
                }),
        );

        let mut req = req;
        if let Some(hooks) = hooks.clone() {
            req = hooks.on_request(req).await?;
        }

        if let VizierRequestContent::SilentRead(_) = req.content {
            if initiative_factor > self.config.silent_read_initiative_chance {
                return Ok(VizierResponse {
                    timestamp: chrono::Utc::now(),
                    content: VizierResponseContent::Empty,
                });
            }
        }

        let (output, stats) = self
            .prompt(req.to_message()?, history, 0, hooks.clone(), false)
            .await?;

        let mut response = VizierResponse {
            timestamp: chrono::Utc::now(),
            content: VizierResponseContent::Message {
                content: output,
                stats: Some(stats),
            },
        };
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
        is_subagent: bool,
    ) -> Result<(String, VizierResponseStats)> {
        timeout(*self.config.prompt_timeout, async {
            let mut history = history.clone();
            let mut turn_depth = turn_depth;
            let max_turn_depth = self.config.thinking_depth;
            let mut tools = self.tools.tools().await?;
            tools.extend(self.skills.get_skills().await?);

            let output: String;

            let mut message = message;

            let start = Instant::now();

            let mut input_tokens: u64 = 0;
            let mut cached_input_tokens: u64 = 0;
            let mut total_cached_input_tokens: u64 = 0;
            let mut total_input_tokens: u64 = 0;
            let mut total_output_tokens: u64 = 0;
            let mut total_tokens: u64 = 0;

            loop {
                turn_depth += 1;
                if max_turn_depth > 0 && turn_depth > max_turn_depth {
                    return Err(anyhow::anyhow!(VizierError(format!(
                        "thinking depth exceeding {}",
                        max_turn_depth
                    ))));
                }

                let (message_id, choices, usage) = self
                    .model
                    .completion(message.clone(), history.clone(), tools.clone())
                    .await?;

                history.push(message);

                history.push(Message::Assistant {
                    id: message_id.clone(),
                    content: choices.clone(),
                });

                if turn_depth == 1 {
                    input_tokens = usage.input_tokens;
                    cached_input_tokens = usage.input_tokens;
                }

                total_input_tokens += usage.input_tokens;
                total_cached_input_tokens += usage.input_tokens;
                total_output_tokens += usage.output_tokens;
                total_tokens += usage.total_tokens;

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
                        }
                    } else {
                        let tool_server = self.tools.clone();
                        match timeout(
                            *self.config.tools.timeout,
                            tokio::spawn(async move {
                                tool_server.call(function_name.clone(), args).await
                            }),
                        )
                        .await??
                        {
                            Err(err) => VizierResponse {
                                timestamp: Utc::now(),
                                content: VizierResponseContent::ToolResponse {
                                    response: serde_json::Value::String(err.to_string()),
                                },
                            },
                            Ok(s) => s,
                        }
                    };

                    if let Some(hooks) = hooks.clone() {
                        tool_res = hooks.on_tool_response(tool_res).await?;
                    }
                    tool_responses.push(
                        tool_res.to_tool_response_content(call.id.clone(), call.call_id.clone())?,
                    );
                }

                message = Message::User {
                    content: OneOrMany::many(tool_responses).unwrap(),
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
                },
            ))
        })
        .await?
    }

    pub async fn call_skill(&self, skill_name: String) -> String {
        let slug = skill_name.replace("SKILL__", "");
        match self.skills.get_skill_content(slug).await {
            Err(err) => err.to_string(),
            Ok(content) => content.unwrap_or("".into()),
        }
    }
}

pub fn read_md_file(workspace: String, file: String) -> String {
    let path = build_path(&workspace, &[&file]);

    fs::read_to_string(path).unwrap()
}
