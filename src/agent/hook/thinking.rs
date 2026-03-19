use anyhow::Result;

use crate::{
    agent::hook::VizierAgentHook,
    schema::{VizierResponse, VizierSession},
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
impl VizierAgentHook for ThinkingHook {
    async fn on_tool_call(&self, function_name: String, args: String) -> Result<(String, String)> {
        self.transport
            .send_response(
                self.session.clone(),
                VizierResponse::Thinking {
                    name: function_name.clone(),
                    args: serde_json::from_str(&args.clone()).unwrap(),
                },
            )
            .await?;

        Ok((function_name, args))
    }

    async fn on_tool_response(&self, res: String) -> Result<String> {
        Ok(res)
    }
}
