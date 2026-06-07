use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolContext, VizierTool},
    dependencies::VizierDependencies,
    error::VizierError,
    schema::{AgentId, VizierRequest, VizierRequestContent, VizierResponseContent, VizierSession},
    transport::VizierTransport,
};

pub struct SubtasksTool {
    agent_id: AgentId,
    transport: VizierTransport,
}

impl SubtasksTool {
    pub fn new(agent_id: AgentId, deps: VizierDependencies) -> Self {
        Self {
            agent_id,
            transport: deps.transport.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct Task {
    prompt: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SubtasksArgs {
    tasks: Vec<Task>,
}

#[async_trait::async_trait]
impl VizierTool for SubtasksTool {
    type Input = SubtasksArgs;
    type Output = Vec<String>;

    fn name() -> String {
        "paralel_subtasks".to_string()
    }

    fn description(&self) -> String {
        "Complete multiple tasks in paralel".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let mut response_rxs = Vec::new();

        for task in &args.tasks {
            let session = VizierSession(
                self.agent_id.clone(),
                crate::schema::VizierChannelId::Subagent,
                Some(uuid::Uuid::new_v4().to_string().to_string()),
            );

            let (response_tx, response_rx) = flume::unbounded();

            let _ = self
                .transport
                .send_request(
                    session.clone(),
                    VizierRequest {
                        timestamp: Utc::now(),
                        user: self.agent_id.clone(),
                        content: VizierRequestContent::Prompt(task.prompt.clone()),
                        metadata: serde_json::json!({}),

                        ..Default::default()
                    },
                    Some(response_tx),
                )
                .await;

            response_rxs.push(response_rx);
        }

        let mut res = vec![];
        for rx in response_rxs {
            loop {
                if let Ok(response) = rx.recv_async().await {
                    if let VizierResponseContent::Message { content, stats: _ } = response.content {
                        res.push(content);
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        Ok(res)
    }
}
