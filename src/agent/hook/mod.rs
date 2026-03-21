use std::sync::Arc;

use crate::schema::{VizierRequest, VizierResponse};
use anyhow::Result;

pub mod history;
pub mod thinking;

#[async_trait::async_trait]
pub trait VizierSessionHook
where
    Self: Send + Sync,
{
    async fn on_request(&self, req: VizierRequest) -> Result<VizierRequest> {
        Ok(req)
    }

    async fn on_response(&self, res: VizierResponse) -> Result<VizierResponse> {
        Ok(res)
    }

    async fn on_tool_call(&self, function_name: String, args: String) -> Result<(String, String)> {
        Ok((function_name, args))
    }

    async fn on_tool_response(&self, res: String) -> Result<String> {
        Ok(res)
    }
}

pub struct VizierSessionHooks(Vec<Arc<Box<dyn VizierSessionHook>>>);

impl VizierSessionHooks {
    pub fn new() -> Self {
        Self(vec![])
    }

    pub fn hook<Hook: VizierSessionHook + 'static>(&mut self, hook: Hook) -> Self {
        let Self(hooks) = self;
        let mut hooks = hooks.clone();
        hooks.push(Arc::new(Box::new(hook)));

        Self(hooks)
    }
}

#[async_trait::async_trait]
impl VizierSessionHook for VizierSessionHooks {
    async fn on_request(&self, req: VizierRequest) -> Result<VizierRequest> {
        let mut final_req = req;
        for hook in self.0.iter() {
            final_req = hook.on_request(final_req.clone()).await?;
        }

        Ok(final_req)
    }

    async fn on_response(&self, res: VizierResponse) -> Result<VizierResponse> {
        let mut final_res = res;
        for hook in self.0.iter() {
            final_res = hook.on_response(final_res.clone()).await?;
        }

        Ok(final_res)
    }

    async fn on_tool_call(&self, function_name: String, args: String) -> Result<(String, String)> {
        let (mut function_name, mut args) = (function_name, args);
        for hook in self.0.iter() {
            (function_name, args) = hook
                .on_tool_call(function_name.clone(), args.clone())
                .await?;
        }

        Ok((function_name, args))
    }

    async fn on_tool_response(&self, res: String) -> Result<String> {
        let mut res = res;
        for hook in self.0.iter() {
            res = hook.on_tool_response(res.clone()).await?;
        }

        Ok(res)
    }
}
