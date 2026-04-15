use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::VizierTool,
    error::VizierError,
    schema::{VizierChannelId, VizierResponse, VizierResponseContent, VizierSession},
    transport::VizierTransport,
};

pub struct WebUiNotifyPrimaryUser {
    agent_id: String,
    transport: VizierTransport,
}

impl WebUiNotifyPrimaryUser {
    pub fn new(agent_id: String, transport: VizierTransport) -> Self {
        Self {
            agent_id,
            transport,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct WebUiNotifyPrimaryUserArgs {
    #[schemars(description = "content of the notification")]
    content: String,
}

#[async_trait::async_trait]
impl VizierTool for WebUiNotifyPrimaryUser
where
    Self: Sync + Send,
{
    type Input = WebUiNotifyPrimaryUserArgs;
    type Output = ();

    fn name() -> String {
        "webui_notify_primary_user".to_string()
    }

    fn description(&self) -> String {
        "send a notification to the primary user via WebUI".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let session = VizierSession(
            self.agent_id.clone(),
            VizierChannelId::HTTP("vizier-webui".to_string()),
            Some("notification".to_string()),
        );

        let response = VizierResponse {
            timestamp: chrono::Utc::now(),
            content: VizierResponseContent::Message {
                content: args.content,
                stats: None,
            },
        };

        match self.transport.send_response(session, response).await {
            Ok(()) => Ok(()),
            Err(err) => {
                log::error!(
                    "webui_notify_primary_user: failed to send notification: {:?}",
                    err
                );
                Ok(())
            }
        }
    }
}
