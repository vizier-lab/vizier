use anyhow::{Ok, Result};
use chrono::Utc;
use rig::embeddings::EmbeddingModel;

use crate::{
    error::VizierError,
    schema::Memory,
    storage::{
        memory::MemoryStorage,
        surreal::{DistanceFunction, SurrealStorage},
    },
};

use slugify::slugify;

#[async_trait::async_trait]
impl MemoryStorage for SurrealStorage {
    async fn write_memory(
        &self,
        agent_id: String,
        slug: Option<String>,
        title: String,
        content: String,
    ) -> Result<()> {
        let embedder = self
            .embedder
            .clone()
            .ok_or(VizierError("embedder is not set".into()))?;

        let slug = slug.unwrap_or_else(|| slugify!(&title));
        let mut memory = Memory {
            slug: slug.clone(),
            agent_id: agent_id.clone(),
            title,
            content: content.clone(),
            timestamp: Utc::now(),
            embedding: vec![],
        };

        let embedding = embedder.embed_text(&content.clone()).await?;
        memory.embedding = embedding.vec;

        let _: Option<Memory> = self
            .conn
            .upsert(("memory", format!("{}/{}", agent_id, slug)))
            .content(memory)
            .await?;

        Ok(())
    }

    async fn query_memory(
        &self,
        agent_id: String,
        query: String,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<Memory>> {
        let embedder = self
            .embedder
            .clone()
            .ok_or(VizierError("embedder is not set".into()))?;

        let query = embedder.embed_text(&query).await?.vec;

        let distance_function = DistanceFunction::Cosine;

        let mut response = self
            .conn
            .query(format!(
                r#"SELECT * 
                    FROM type::table($table) 
                    WHERE {distance_function}($query, embedding) >= $threshold AND agent_id = $agent_id
                    ORDER BY distance ASC 
                    LIMIT $limit"#
            ))
            .bind(("table", "memory"))
            .bind(("agent_id", agent_id))
            .bind(("query", query))
            .bind(("limit", limit))
            .bind(("threshold", threshold))
            .await?;

        let res: Vec<Memory> = response.take(0)?;

        Ok(res)
    }

    async fn get_all_agent_memory(&self, agent_id: String) -> Result<Vec<Memory>> {
        let mut response = self
            .conn
            .query("SELECT * FROM type::table(memory) WHERE agent_id = $agent_id")
            .bind(("agent_id", agent_id))
            .await?;

        let data: Vec<Memory> = response.take(0).unwrap();

        Ok(data)
    }

    async fn get_memory_detail(&self, agent_id: String, slug: String) -> Result<Option<Memory>> {
        let mut response = self
            .conn
            .query("SELECT * FROM type::table(memory) WHERE slug = $slug AND agent_id = $agent_id")
            .bind(("slug", slug))
            .bind(("agent_id", agent_id))
            .await?;

        let data: Option<Memory> = response.take(0)?;

        Ok(data)
    }

    async fn delete_memory(&self, agent_id: String, slug: String) -> Result<()> {
        let _ = self
            .conn
            .delete::<Option<Memory>>(("memory", format!("{}/{}", agent_id, slug.clone())))
            .await?;

        Ok(())
    }
}
