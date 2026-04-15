use std::collections::HashMap;

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serenity::async_trait;

use crate::agents::tools::VizierTool;
use crate::config::agent::AgentConfig;
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::{
    AgentId, TopicId, VizierChannelId, VizierRequest, VizierRequestContent, VizierResponse,
    VizierResponseContent, VizierSession,
};
use crate::transport::VizierTransport;

pub struct ConsultAgent {
    agent_id: String,
    agents: HashMap<String, AgentConfig>,
    transport: VizierTransport,
}

impl ConsultAgent {
    pub fn new(agent_id: AgentId, deps: VizierDependencies) -> Self {
        let agents = deps
            .config
            .agents
            .clone()
            .iter()
            .filter_map(|(aid, config)| {
                if &agent_id == aid {
                    None
                } else {
                    Some((aid.clone(), config.clone()))
                }
            })
            .collect::<HashMap<_, _>>();
        Self {
            agent_id,
            agents,
            transport: deps.transport.clone(),
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

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let mut recv = self
            .transport
            .subscribe_response()
            .await
            .map_err(|err| VizierError(err.to_string()))?;

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
                },
            )
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        loop {
            let (session, response) = recv
                .recv()
                .await
                .map_err(|err| VizierError(err.to_string()))?;

            if session != curr_session {
                continue;
            }

            if let VizierResponse {
                content: VizierResponseContent::Message { content, stats: _ },
                timestamp: _,
            } = response
            {
                return Ok(content);
            }
        }
    }
}

pub struct DelegateAgent {
    agent_id: String,
    agents: HashMap<String, AgentConfig>,
    transport: VizierTransport,
}

impl DelegateAgent {
    pub fn new(agent_id: AgentId, deps: VizierDependencies) -> Self {
        let agents = deps
            .config
            .agents
            .clone()
            .iter()
            .filter_map(|(aid, config)| {
                if &agent_id == aid {
                    None
                } else {
                    Some((aid.clone(), config.clone()))
                }
            })
            .collect::<HashMap<_, _>>();
        Self {
            agent_id,
            agents,
            transport: deps.transport.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DelegateAgentArgs {
    #[schemars(description = "agent_id of the target agent")]
    pub agent_id: String,
    #[schemars(description = "task for the agent")]
    pub prompt: String,
}

#[async_trait::async_trait]
impl VizierTool for DelegateAgent {
    type Input = DelegateAgentArgs;
    type Output = ();

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
            "Assign an agent a task to do, this is a non-blocking tool, you won't need to wait the agent\n\nAvailable Agent\n{available_agents_desc}"
        )
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let curr_session = VizierSession(
            args.agent_id.clone(),
            VizierChannelId::InterAgent(vec![self.agent_id.clone(), args.agent_id.clone()]),
            None,
        );

        let _ = self
            .transport
            .send_request(
                curr_session.clone(),
                VizierRequest {
                    timestamp: chrono::Utc::now(),
                    user: self.agent_id.clone(),
                    content: VizierRequestContent::Prompt(args.prompt),
                    metadata: json!({}),
                },
            )
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        Ok(())
    }
}

