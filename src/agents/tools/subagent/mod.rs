use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::{AgentId, VizierRequest, VizierRequestContent, VizierResponse, VizierSession};
use crate::transport::VizierTransport;

pub struct SpawnSubAgents {
    agent_id: String,
    transport: VizierTransport,
}

impl SpawnSubAgents {
    pub fn new(agent_id: AgentId, deps: VizierDependencies) -> Self {
        Self {
            agent_id,
            transport: deps.transport.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct Subtask {
    pub title: String,
    pub prompt: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SubAgentArgs {
    tasks: Vec<Subtask>,
}

impl Tool for SpawnSubAgents {
    const NAME: &'static str = "spawn_subagents";
    type Error = VizierError;
    type Args = SubAgentArgs;
    type Output = Vec<Result<String, String>>;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let parameters = serde_json::to_value(schema_for!(Self::Args)).unwrap();

        ToolDefinition {
            name: Self::NAME.to_string(),
            description: format!(
                "spawn subagents of yourselves a wait for them to do one or multiple task in paralel"
            ),
            parameters,
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut js = JoinSet::<Result<VizierResponse, VizierError>>::new();
        for task in args.tasks {
            let agent_id = self.agent_id.clone();
            let session = VizierSession(
                agent_id.clone(),
                crate::schema::VizierChannelId::Subagent,
                Some(task.title.clone()),
            );

            self.transport
                .send_request(
                    session.clone(),
                    VizierRequest {
                        user: agent_id.clone(),
                        content: VizierRequestContent::Prompt(task.prompt.clone()),
                        metadata: serde_json::json!({}),
                    },
                )
                .await
                .map_err(|err| VizierError(err.to_string()))?;

            let mut recv = self
                .transport
                .subscribe_response()
                .await
                .map_err(|err| VizierError(err.to_string()))?;

            js.spawn(async move {
                loop {
                    if let Ok((curr_session, response)) = recv.recv().await {
                        if session == curr_session {
                            return Ok(response);
                        }
                    }
                }
            });
        }

        let results = js.join_all().await;

        let mut res = vec![];
        for result in results.iter() {
            match result {
                Ok(VizierResponse::Message { content, stats: _ }) => {
                    res.push(Ok(content.clone()));
                }
                Err(err) => {
                    res.push(Err(format!("{:?}", err)));
                }
                _ => {}
            }
        }

        Ok(res)
    }
}
