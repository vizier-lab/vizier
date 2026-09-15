use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolContext, VizierTool},
    error::VizierError,
    schema::RevisionOrigin,
    storage::{VizierStorage, agent::AgentStorage},
};

pub struct CoreDocument;

pub struct WriteCore {
    agent_id: String,
    storage: Arc<VizierStorage>,
}

impl WriteCore {
    pub fn new(agent_id: String, storage: Arc<VizierStorage>) -> Self {
        Self { agent_id, storage }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct WriteCoreArgs {
    #[schemars(description = "New content for the CORE document")]
    content: String,
}

#[async_trait::async_trait]
impl VizierTool for WriteCore {
    type Input = WriteCoreArgs;
    type Output = String;

    fn name() -> String {
        "WRITE_CORE".to_string()
    }

    fn description(&self) -> String {
        "write over your CORE document, **not append**. Always tell user after updating!".to_string()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        self.storage
            .set_agent_core(
                &self.agent_id,
                &args.content,
                &RevisionOrigin::from_session(&ctx.session),
            )
            .await
            .map_err(|err| VizierError(err.to_string()))?;
        Ok("CORE updated successfully".to_string())
    }
}

pub struct ReadCore {
    agent_id: String,
    storage: Arc<VizierStorage>,
}

impl ReadCore {
    pub fn new(agent_id: String, storage: Arc<VizierStorage>) -> Self {
        Self { agent_id, storage }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ReadCoreArgs {}

#[async_trait::async_trait]
impl VizierTool for ReadCore {
    type Input = ReadCoreArgs;
    type Output = String;

    fn name() -> String {
        "READ_CORE".to_string()
    }

    fn description(&self) -> String {
        "read your CORE document".to_string()
    }

    async fn call(
        &self,
        _args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let content = self
            .storage
            .get_agent_core(&self.agent_id)
            .await
            .map_err(|err| VizierError(err.to_string()))?
            .unwrap_or_default();
        Ok(content)
    }
}
