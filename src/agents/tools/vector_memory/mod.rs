use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::agents::tools::VizierTool;
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::AgentId;
use crate::storage::VizierStorage;
use crate::storage::memory::MemoryStorage;

pub fn init_vector_memory(
    agent_id: String,
    deps: VizierDependencies,
) -> Result<(MemoryRead, MemoryWrite)> {
    Ok((
        MemoryRead::new(agent_id.clone(), deps.storage.clone()),
        MemoryWrite::new(agent_id.clone(), deps.storage.clone()),
    ))
}

pub type MemoryRead = ReadVectorMemory;
pub struct ReadVectorMemory(AgentId, Arc<VizierStorage>);

impl MemoryRead {
    fn new(agent_id: AgentId, store: Arc<VizierStorage>) -> Self {
        Self(agent_id, store)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryReadArgs {
    #[schemars(description = "Terms, keywords, or prompt to search")]
    pub query: String,
}

#[async_trait::async_trait]
impl VizierTool for MemoryRead {
    type Input = MemoryReadArgs;
    type Output = Vec<String>;

    fn name() -> String {
        "memory_read".to_string()
    }

    fn description(&self) -> String {
        "Search your memory for informations".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let res = self
            .1
            .query_memory(self.0.clone(), args.query, 10, 0.1)
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        Ok(res.iter().map(|memory| memory.content.clone()).collect())
    }
}

pub type MemoryWrite = WriteVectorMemory;
pub struct WriteVectorMemory(AgentId, Arc<VizierStorage>);

impl MemoryWrite {
    fn new(agent_id: AgentId, store: Arc<VizierStorage>) -> Self {
        Self(agent_id, store)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, Clone)]
pub struct MemoryWriteArgs {
    #[schemars(description = "title of the memory")]
    pub title: String,

    #[schemars(description = "details of the memory")]
    pub content: String,
}

#[async_trait::async_trait]
impl VizierTool for MemoryWrite {
    type Input = MemoryWriteArgs;
    type Output = String;

    fn name() -> String {
        "memory_write".to_string()
    }

    fn description(&self) -> String {
        "write or update a new memory".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let slug = slugify!(&args.title).to_string();

        let content = format!(
            "#{}\n\n{}\n\n timestamp: {}",
            args.title,
            args.content,
            Utc::now()
        );

        let _ = self
            .1
            .write_memory(self.0.clone(), Some(slug.clone()), args.title, content)
            .await;

        Ok(format!("memory {slug} is written"))
    }
}
