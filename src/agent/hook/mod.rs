use crate::schema::{VizierRequest, VizierResponse};
use anyhow::Result;

pub mod thinking;

#[async_trait::async_trait]
pub trait VizierAgentHook
where
    Self: Send + Sync,
{
    async fn on_request(&self, req: VizierRequest) -> Result<VizierRequest> {
        Ok(req)
    }

    async fn on_response(&self, res: String) -> Result<String> {
        Ok(res)
    }

    async fn on_tool_call(&self, function_name: String, args: String) -> Result<(String, String)> {
        Ok((function_name, args))
    }

    async fn on_tool_response(&self, res: String) -> Result<String> {
        Ok(res)
    }
}
