use anyhow::Result;
use serde_json::Value;

use crate::{
    agents::hook::VizierSessionHook,
    schema::{VizierResponse, VizierResponseContent, VizierSession},
    transport::VizierTransport,
};

#[derive(Debug, Clone)]
pub struct ToolCallsHook {
    transport: VizierTransport,
    session: VizierSession,
}

impl ToolCallsHook {
    pub fn new(transport: VizierTransport, session: VizierSession) -> Self {
        Self { transport, session }
    }
}

#[async_trait::async_trait]
impl VizierSessionHook for ToolCallsHook {
    async fn on_tool_call(&self, function_name: String, args: String) -> Result<(String, String)> {
        if function_name != "think" {
            let args_json: serde_json::Value = serde_json::from_str::<Value>(&args)?;
            self.transport
                .send_response(
                    self.session.clone(),
                    VizierResponse {
                        timestamp: chrono::Utc::now(),
                        content: VizierResponseContent::ToolChoice {
                            name: function_name.clone(),
                            args: args_json,
                        },
                    },
                )
                .await?;
        }

        Ok((function_name, args))
    }

    async fn on_tool_response(&self, res: VizierResponse) -> Result<VizierResponse> {
        Ok(res)
    }
}
