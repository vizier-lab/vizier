use anyhow::Result;
use serde_json::Value;

use crate::{
    agents::hook::VizierSessionHook,
    schema::{VizierResponse, VizierResponseContent, VizierSession},
};

#[derive(Debug, Clone)]
pub struct ToolCallsHook {
    response_tx: flume::Sender<VizierResponse>,
    session: VizierSession,
}

impl ToolCallsHook {
    pub fn new(response_tx: flume::Sender<VizierResponse>, session: VizierSession) -> Self {
        Self {
            response_tx,
            session,
        }
    }
}

#[async_trait::async_trait]
impl VizierSessionHook for ToolCallsHook {
    async fn on_tool_call(&self, function_name: String, args: String) -> Result<(String, String)> {
        if function_name != "think" {
            let args_json: serde_json::Value = serde_json::from_str::<Value>(&args)?;
            let _ = self
                .response_tx
                .send_async(VizierResponse {
                    timestamp: chrono::Utc::now(),
                    content: VizierResponseContent::ToolChoice {
                        name: function_name.clone(),
                        args: args_json,
                    },
                    attachments: vec![],
                })
                .await;
        }

        Ok((function_name, args))
    }

    async fn on_tool_response(&self, res: VizierResponse) -> Result<VizierResponse> {
        Ok(res)
    }
}
