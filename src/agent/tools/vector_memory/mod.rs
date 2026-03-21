use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::dependencies::VizierDependencies;
use crate::embedding;
use crate::error::VizierError;
use crate::schema::{AgentId, Memory};
use crate::storage::VizierStorage;
use crate::storage::memory::MemoryStorage;

pub fn init_vector_memory(
    agent_id: String,
    deps: VizierDependencies,
) -> Result<(MemoryRead, MemoryWrite)> {
    let embedder = deps.embedder.unwrap();
    Ok((
        MemoryRead::new(agent_id.clone(), deps.storage.clone(), embedder.clone()),
        MemoryWrite::new(agent_id.clone(), deps.storage.clone(), embedder.clone()),
    ))
}

pub type MemoryRead = ReadVectorMemory;
pub struct ReadVectorMemory(AgentId, VizierStorage, Arc<embedding::EmbeddingModel>);

impl MemoryRead {
    fn new(agent_id: AgentId, store: VizierStorage, model: Arc<embedding::EmbeddingModel>) -> Self {
        Self(agent_id, store, model)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryReadArgs {
    #[schemars(description = "Terms, keywords, or prompt to search")]
    pub query: String,
}

impl Tool for MemoryRead {
    const NAME: &'static str = "memory_read";
    type Error = VizierError;
    type Args = MemoryReadArgs;
    type Output = Vec<Memory>;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let parameters = serde_json::to_value(schema_for!(Self::Args)).unwrap();

        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Search your memory for informations".into(),
            parameters,
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        log::info!("read_memory: {}", args.query.clone());

        let res = self
            .1
            .query_memory(self.0.clone(), args.query, 10, 0.)
            .await
            .unwrap();

        Ok(res)
    }
}

pub type MemoryWrite = WriteVectorMemory;
pub struct WriteVectorMemory(AgentId, VizierStorage, Arc<embedding::EmbeddingModel>);

impl MemoryWrite {
    fn new(agent_id: AgentId, store: VizierStorage, model: Arc<embedding::EmbeddingModel>) -> Self {
        Self(agent_id, store, model)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, Clone)]
pub struct MemoryWriteArgs {
    #[schemars(description = "title of the memory")]
    pub title: String,

    #[schemars(description = "details of the memory")]
    pub content: String,
}

impl Tool for MemoryWrite {
    const NAME: &'static str = "memory_write";
    type Error = VizierError;
    type Args = MemoryWriteArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let parameters = serde_json::to_value(schema_for!(Self::Args)).unwrap();

        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "write or update a new memory".into(),
            parameters,
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let slug = slugify!(&args.title).to_string();
        log::info!("write_memory: {:?}", slug.clone());

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
