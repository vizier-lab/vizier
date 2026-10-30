use anyhow::Result;
use serde_json::Value;

use crate::{
    agents::hook::VizierSessionHook,
    schema::{VizierResponse, VizierResponseContent, VizierSession},
};

#[derive(Debug, Clone)]
pub struct ThinkingHook {
    response_tx: flume::Sender<VizierResponse>,
    session: VizierSession,
}

impl ThinkingHook {
    pub fn new(response_tx: flume::Sender<VizierResponse>, session: VizierSession) -> Self {
        Self {
            response_tx,
            session,
        }
    }
}

#[async_trait::async_trait]
impl VizierSessionHook for ThinkingHook {
    async fn on_tool_call(&self, function_name: String, args: String) -> Result<(String, String)> {
        if function_name == "think".to_string() {
            if let Ok(thinking) = serde_json::from_str::<Value>(&args) {
                let _ = self
                    .response_tx
                    .send_async(VizierResponse {
                        timestamp: chrono::Utc::now(),
                        content: VizierResponseContent::Thinking(
                            thinking["thought"].as_str().unwrap().to_string(),
                        ),
                        attachments: vec![],
                        ..Default::default()
                    })
                    .await;
            }
        }

        Ok((function_name, args))
    }

    /// A script calling `think` is as script-internal as any other nested call, so it does
    /// not stream either.
    async fn on_nested_tool_call(
        &self,
        function_name: String,
        args: String,
    ) -> Result<(String, String)> {
        Ok((function_name, args))
    }
}
