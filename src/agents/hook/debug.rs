use anyhow::Result;

use crate::{
    agents::hook::VizierSessionHook,
    schema::{VizierRequest, VizierResponse, VizierSession},
};

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
        tracing::debug!("{:?} tool call: {} {}", self.0, function_name, args);
        Ok((function_name, args))
    }

    async fn on_tool_response(&self, res: VizierResponse) -> Result<VizierResponse> {
        tracing::debug!("tool resp: {:?} {:?}", self.0, res);
        Ok(res)
    }
}
