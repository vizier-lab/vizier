use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::agents::background::{PieceSpec, validate_timeout};
use crate::agents::tools::{ToolContext, VizierTool};
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::{
    AgentConfig, AgentId, JobKind, TopicId, VizierChannelId, VizierRequest, VizierRequestContent,
    VizierResponse, VizierResponseContent, VizierSession,
};
use crate::transport::VizierTransport;

pub struct ConsultAgent {
    agent_id: String,
    agents: HashMap<String, AgentConfig>,
    transport: VizierTransport,
}

impl ConsultAgent {
    pub fn new(agent_id: AgentId, agents: HashMap<String, AgentConfig>, transport: VizierTransport) -> Self {
        Self {
            agent_id,
            agents,
            transport,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ConsultAgentArgs {
    #[schemars(
        description = "[optional] identifier for current topic/conversation, the consult session will be ephemeral if left empty"
    )]
    pub topic_id: Option<TopicId>,
    #[schemars(description = "agent_id of the target agent")]
    pub agent_id: String,
    #[schemars(description = "Question, task, or discussion to ask the agent")]
    pub prompt: String,
}

#[async_trait::async_trait]
impl VizierTool for ConsultAgent {
    type Input = ConsultAgentArgs;
    type Output = String;

    fn name() -> String {
        "consult_agent".to_string()
    }

    fn description(&self) -> String {
        "Consult, or ask other agent and wait for the response".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let (response_tx, response_rx) = flume::unbounded();

        let curr_session = VizierSession(
            args.agent_id.clone(),
            VizierChannelId::InterAgent(vec![self.agent_id.clone(), args.agent_id.clone()]),
            args.topic_id,
        );

        let _ = self
            .transport
            .send_request(
                curr_session.clone(),
                VizierRequest {
                    timestamp: chrono::Utc::now(),
                    user: self.agent_id.clone(),
                    content: VizierRequestContent::Chat(args.prompt.clone()),
                    metadata: json!({}),

                    ..Default::default()
                },
                Some(response_tx),
            )
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        loop {
            let response = response_rx
                .recv_async()
                .await
                .map_err(|err| VizierError(err.to_string()))?;

            if let VizierResponse {
                content: VizierResponseContent::Message { content, stats: _ },
                timestamp: _,
                attachments: _,
            } = response
            {
                return Ok(content);
            }
        }
    }
}

pub struct DelegateAgent {
    agents: HashMap<String, AgentConfig>,
    deps: VizierDependencies,
}

impl DelegateAgent {
    pub fn new(agents: HashMap<String, AgentConfig>, deps: VizierDependencies) -> Self {
        Self { agents, deps }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DelegateAgentArgs {
    #[schemars(description = "agent_id of the target agent")]
    pub agent_id: String,
    #[schemars(description = "task for the agent")]
    pub prompt: String,
    #[schemars(
        description = "[optional] time limit for the task, in seconds, from 1 to 3600 (default 600)"
    )]
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

#[async_trait::async_trait]
impl VizierTool for DelegateAgent {
    type Input = DelegateAgentArgs;
    type Output = String;

    fn name() -> String {
        "delegate_agent".to_string()
    }

    fn description(&self) -> String {
        let available_agents_desc = self
            .agents
            .iter()
            .map(|(agent_id, config)| {
                format!(
                    r#"**Agent ID:** {}
**Name:** {}
**Description:** {}"#,
                    agent_id,
                    config.name,
                    config.description.clone().unwrap_or("".into())
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "Hand a task to another agent, in the background. This call returns immediately with a job id. When the other agent has answered, you will receive its answer as a background report in this same conversation. Do not wait or poll for it.\n\nAvailable Agent\n{available_agents_desc}"
        )
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let timeout_secs = validate_timeout(args.timeout_secs)?;

        let target = args.agent_id;
        if !self.agents.contains_key(&target)
            || !self.deps.transport.is_agent_registered(&target).await
        {
            return Err(VizierError(format!(
                "agent '{target}' not found or not running"
            )));
        }

        let job = self
            .deps
            .background_jobs
            .launch(
                ctx,
                JobKind::Delegation,
                vec![PieceSpec {
                    executor_agent: target.clone(),
                    prompt: args.prompt,
                }],
                timeout_secs,
            )
            .await?;

        Ok(format!(
            "Delegated to agent '{}' as background job {}. Its answer will arrive as a background report in this conversation.",
            target, job.id
        ))
    }
}
