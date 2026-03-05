use std::sync::Arc;

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use rig::vector_store::VectorStoreIndex;
use rig::vector_store::request::VectorSearchRequest;
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::config::VectorMemoryConfig;
use crate::database::schema::Memory;
use crate::database::{DistanceFunction, VizierDatabases};
use crate::dependencies::VizierDependencies;
use crate::embedding;
use crate::error::VizierError;

pub async fn init_vector_memory(
    workspace: String,
    config: VectorMemoryConfig,
    deps: VizierDependencies,
) -> Result<(MemoryRead, MemoryWrite)> {
    let embedder = Arc::new(
        embedding::Client::new().embedding_model(&config.model.to_fastembed(), Some(workspace)),
    );

    Ok((
        MemoryRead::new(deps.database.clone(), embedder.clone()),
        MemoryWrite::new(deps.database.clone(), embedder.clone()),
    ))
}

pub type MemoryRead = ReadVectorMemory;
pub struct ReadVectorMemory(VizierDatabases, Arc<embedding::EmbeddingModel>);

impl MemoryRead {
    fn new(store: VizierDatabases, model: Arc<embedding::EmbeddingModel>) -> Self {
        Self(store, model)
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
            .0
            .query_memory(&self.1, args.query, DistanceFunction::Euclidean, 10, 0.)
            .await
            .unwrap();

        Ok(res)
    }
}

pub type MemoryWrite = WriteVectorMemory;
pub struct WriteVectorMemory(VizierDatabases, Arc<embedding::EmbeddingModel>);

impl MemoryWrite {
    fn new(store: VizierDatabases, model: Arc<embedding::EmbeddingModel>) -> Self {
        Self(store, model)
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

        let _ = self
            .0
            .write_memory(&self.1, Some(slug.clone()), args.title, args.content)
            .await;

        Ok(format!("memory {slug} is written"))
    }
}
