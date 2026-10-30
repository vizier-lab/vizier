use anyhow::Result;
use serde_json::Value;

use crate::{
    agents::hook::VizierSessionHook,
    sandbox::ExecutionReport,
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
                    ..Default::default()
                })
                .await;
        }

        Ok((function_name, args))
    }

    /// A script's own tool calls are not streamed. The report the script's run returns is
    /// what the agent reads; the individual calls are the script's business, and one
    /// `ToolChoice` frame per iteration of a loop is noise in the transcript.
    async fn on_nested_tool_call(
        &self,
        function_name: String,
        args: String,
    ) -> Result<(String, String)> {
        Ok((function_name, args))
    }

    async fn on_tool_response(&self, res: VizierResponse) -> Result<VizierResponse> {
        // Only execute_python's report is forwarded live. Identified by shape: nested
        // calls from a script pass through this hook too, so "last tool name seen"
        // would point at the last nested tool, not at execute_python.
        if let VizierResponseContent::ToolResponse { response } = &res.content
            && ExecutionReport::looks_like(response)
        {
            let _ = self.response_tx.send_async(res.clone()).await;
        }

        Ok(res)
    }
}
