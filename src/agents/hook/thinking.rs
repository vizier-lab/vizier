use anyhow::Result;
use serde_json::Value;

use crate::{
    agents::hook::VizierSessionHook,
    schema::{VizierResponse, VizierResponseContent, VizierSession},
    transport::VizierTransport,
};

#[derive(Debug, Clone)]
pub struct ThinkingHook {
    transport: VizierTransport,
    session: VizierSession,
}

impl ThinkingHook {
    pub fn new(transport: VizierTransport, session: VizierSession) -> Self {
        Self { transport, session }
    }
}

#[async_trait::async_trait]
impl VizierSessionHook for ThinkingHook {
    async fn on_tool_call(&self, function_name: String, args: String) -> Result<(String, String)> {
        if function_name == "think".to_string() {
            if let Ok(thinking) = serde_json::from_str::<Value>(&args) {
                self.transport
                    .send_response(
                        self.session.clone(),
                        VizierResponse {
                            timestamp: chrono::Utc::now(),
                            content: VizierResponseContent::Thinking(
                                thinking["thought"].as_str().unwrap().to_string(),
                            ),
                        },
                    )
                    .await?;
            }
        }

        Ok((function_name, args))
    }
}
