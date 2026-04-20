use anyhow::Result;

use crate::{
    agents::hook::VizierSessionHook,
    schema::{VizierResponse, VizierSession},
};

pub struct DebugHook(pub VizierSession);

#[async_trait::async_trait]
impl VizierSessionHook for DebugHook {
    async fn on_tool_call(&self, function_name: String, args: String) -> Result<(String, String)> {
        log::debug!("{:?} tool call: {} {}", self.0, function_name, args);
        Ok((function_name, args))
    }

    async fn on_tool_response(&self, res: VizierResponse) -> Result<VizierResponse> {
        log::debug!("tool resp: {:?}", res);
        Ok(res)
    }
}
