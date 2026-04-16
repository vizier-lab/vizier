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
) -> Result<(MemoryRead, MemoryWrite, MemoryList, MemoryDetail)> {
    Ok((
        MemoryRead::new(agent_id.clone(), deps.storage.clone()),
        MemoryWrite::new(agent_id.clone(), deps.storage.clone()),
        MemoryList::new(agent_id.clone(), deps.storage.clone()),
        MemoryDetail::new(agent_id.clone(), deps.storage.clone()),
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
pub struct MemoryListArgs {
    #[schemars(description = "Maximum number of memories to return")]
    #[serde(default = "default_limit")]
    pub limit: Option<usize>,

    #[schemars(description = "Number of memories to skip")]
    #[serde(default = "default_offset")]
    pub offset: Option<usize>,
}

fn default_limit() -> Option<usize> {
    Some(50)
}

fn default_offset() -> Option<usize> {
    Some(0)
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemorySummary {
    pub slug: String,
    pub title: String,
    pub timestamp: chrono::DateTime<Utc>,
}

pub type MemoryList = ListVectorMemory;
pub struct ListVectorMemory(AgentId, Arc<VizierStorage>);

impl MemoryList {
    fn new(agent_id: AgentId, store: Arc<VizierStorage>) -> Self {
        Self(agent_id, store)
    }
}

#[async_trait::async_trait]
impl VizierTool for MemoryList {
    type Input = MemoryListArgs;
    type Output = Vec<MemorySummary>;

    fn name() -> String {
        "memory_list".to_string()
    }

    fn description(&self) -> String {
        "List all available memories".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let limit = args.limit.unwrap_or(50);
        let offset = args.offset.unwrap_or(0);

        let all_memory = self
            .1
            .get_all_agent_memory(self.0.clone())
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        Ok(all_memory
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|m| MemorySummary {
                slug: m.slug,
                title: m.title,
                timestamp: m.timestamp,
            })
            .collect())
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

pub type MemoryDetail = GetVectorMemory;
pub struct GetVectorMemory(AgentId, Arc<VizierStorage>);

impl MemoryDetail {
    fn new(agent_id: AgentId, store: Arc<VizierStorage>) -> Self {
        Self(agent_id, store)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryDetailArgs {
    #[schemars(description = "Slug of the memory to retrieve")]
    pub slug: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryDetailOutput {
    pub slug: String,
    pub title: String,
    pub content: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub agent_id: String,
}

#[async_trait::async_trait]
impl VizierTool for MemoryDetail {
    type Input = MemoryDetailArgs;
    type Output = Option<MemoryDetailOutput>;

    fn name() -> String {
        "memory_detail".to_string()
    }

    fn description(&self) -> String {
        "Get memory details by slug".into()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let memory = self
            .1
            .get_memory_detail(self.0.clone(), args.slug)
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        Ok(memory.map(|m| MemoryDetailOutput {
            slug: m.slug,
            title: m.title,
            content: m.content,
            timestamp: m.timestamp,
            agent_id: m.agent_id,
        }))
    }
}
