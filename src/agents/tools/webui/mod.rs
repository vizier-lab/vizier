use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::agents::tools::{ToolContext, VizierTool};
use crate::error::VizierError;
use crate::schema::{
    AgentId, VizierChannelId, VizierResponse, VizierResponseContent, VizierSession,
};
use crate::storage::{VizierStorage, history::HistoryStorage, session::SessionStorage};

pub struct SendWebuiMessage {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SendWebuiMessageArgs {
    #[schemars(description = "username of the target webui user")]
    username: String,

    #[schemars(description = "id of the target topic")]
    topic_id: String,

    #[schemars(description = "content of the message")]
    content: String,
}

#[async_trait::async_trait]
impl VizierTool for SendWebuiMessage {
    type Input = SendWebuiMessageArgs;
    type Output = String;

    fn name() -> String {
        "webui_send_message".to_string()
    }

    fn description(&self) -> String {
        "send a message to a webui user's topic, avoid using this when user interact with you directly from webui".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let session = VizierSession(
            self.agent_id.clone(),
            VizierChannelId::HTTP(args.username.clone(), "vizier-webui".to_string()),
            Some(args.topic_id.clone()),
        );

        let response = VizierResponse {
            timestamp: Utc::now(),
            content: VizierResponseContent::Message {
                content: args.content,
                stats: None,
            },
            attachments: vec![],
        };

        self.storage
            .save_session_history(session, crate::schema::SessionHistoryContent::Response(response))
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        Ok(format!(
            "Message sent to user {} topic {}",
            args.username, args.topic_id
        ))
    }
}

pub struct ListWebuiTopics {
    pub storage: Arc<VizierStorage>,
    pub agent_id: AgentId,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListWebuiTopicsArgs {
    #[schemars(description = "username of the target webui user")]
    username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebuiTopicEntry {
    #[schemars(description = "topic id")]
    pub topic_id: String,

    #[schemars(description = "topic title")]
    pub title: String,

    #[schemars(description = "whether the agent is currently thinking in this topic")]
    pub is_thinking: bool,
}

#[async_trait::async_trait]
impl VizierTool for ListWebuiTopics {
    type Input = ListWebuiTopicsArgs;
    type Output = Vec<WebuiTopicEntry>;

    fn name() -> String {
        "webui_list_topics".to_string()
    }

    fn description(&self) -> String {
        "list all webui topics for a given user".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let channel = VizierChannelId::HTTP(args.username, "vizier-webui".to_string());

        let sessions = self
            .storage
            .get_session_list(self.agent_id.clone(), Some(channel))
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        let topics: Vec<WebuiTopicEntry> = sessions
            .into_iter()
            .filter_map(|s| {
                let topic_id = s.topic?;
                Some(WebuiTopicEntry {
                    topic_id,
                    title: s.title,
                    is_thinking: s.is_thinking,
                })
            })
            .collect();

        Ok(topics)
    }
}
