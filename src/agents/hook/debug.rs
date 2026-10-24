use anyhow::Result;

use crate::{
    agents::hook::VizierSessionHook,
    schema::{VizierRequest, VizierResponse, VizierResponseContent, VizierSession},
};

/// Logs the shape of a turn, never its payload (`specs/027-agent-secrets` T025): a tool's
/// arguments or response can carry a credential, and this hook sees them before anything
/// redacts. Requests and responses (the person's own words and the agent's reply) stay.
pub struct DebugHook(pub VizierSession);

#[async_trait::async_trait]
impl VizierSessionHook for DebugHook {
    async fn on_request(&self, req: VizierRequest) -> Result<VizierRequest> {
        tracing::debug!("[Request]: {:?} {:?}", self.0, req);
        Ok(req)
    }

    async fn on_response(&self, res: VizierResponse) -> Result<VizierResponse> {
        tracing::debug!("[Response]: {:?} {:?}", self.0, res);
        Ok(res)
    }

    async fn on_tool_call(&self, function_name: String, args: String) -> Result<(String, String)> {
        tracing::debug!("{:?} tool call: {}", self.0, function_name);
        Ok((function_name, args))
    }

    async fn on_tool_response(&self, res: VizierResponse) -> Result<VizierResponse> {
        // The variant, not the payload: a `ToolResponse`'s value is exactly what must not
        // reach a log.
        tracing::debug!("{:?} tool response: {}", self.0, content_kind(&res.content));
        Ok(res)
    }
}

fn content_kind(content: &VizierResponseContent) -> &'static str {
    match content {
        VizierResponseContent::ThinkingStart => "thinking_start",
        VizierResponseContent::Thinking(_) => "thinking",
        VizierResponseContent::ToolChoice { .. } => "tool_choice",
        VizierResponseContent::ToolResponse { .. } => "tool_response",
        VizierResponseContent::Message { .. } => "message",
        VizierResponseContent::AudioReply(..) => "audio_reply",
        VizierResponseContent::Error { .. } => "error",
        VizierResponseContent::Empty => "empty",
        VizierResponseContent::Abort => "abort",
        VizierResponseContent::Checkpoint { .. } => "checkpoint",
    }
}
