use std::collections::HashMap;

use chrono::Utc;
use rig::{completion::ToolDefinition, tool::Tool};
use schemars::schema_for;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::VizierTool,
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

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let mut recv = self
            .transport
            .subscribe_response()
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        let mut sessions = HashMap::new();

        let mut res = vec![];
        for task in &args.tasks {
            let session = VizierSession(
                self.agent_id.clone(),
                crate::schema::VizierChannelId::Subagent,
                Some(uuid::Uuid::new_v4().to_string().to_string()),
            );

            let _ = self
                .transport
                .send_request(
                    session.clone(),
                    VizierRequest {
                        timestamp: Utc::now(),
                        user: self.agent_id.clone(),
                        content: VizierRequestContent::Prompt(task.prompt.clone()),
                        metadata: serde_json::json!({}),
                    },
                )
                .await;

            sessions.insert(session, true);
        }

        loop {
            if res.len() == args.tasks.len() {
                break;
            }

            if let Ok((session, response)) = recv.recv().await {
                if sessions.get(&session).is_none() {
                    continue;
                }

                if let VizierResponseContent::Message { content, stats: _ } = response.content {
                    res.push(content);
                }
            }
        }

        Ok(res)
    }
}
