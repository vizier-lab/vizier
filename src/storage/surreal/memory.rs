use anyhow::Result;
use chrono::Utc;

use crate::{
    embedding::VizierEmbeddingModel,
    error::VizierError,
    schema::{Memory, MemoryVisibility},
    storage::{
        memory::MemoryStorage,
        surreal::{DistanceFunction, SurrealStorage},
    },
};

use slugify::slugify;

const GLOBAL_AGENT_ID: &str = "_global";

#[async_trait::async_trait]
impl MemoryStorage for SurrealStorage {
    async fn write_memory(
        &self,
        agent_id: String,
        slug: Option<String>,
        title: String,
        content: String,
        visibility: MemoryVisibility,
        shared_to: Vec<String>,
    ) -> Result<()> {
        let embedder = self
            .embedder
            .clone()
            .ok_or(VizierError("embedder is not set".into()))?;

        let slug = slug.unwrap_or_else(|| slugify!(&title));
        let store_agent_id = match visibility {
            MemoryVisibility::Global => GLOBAL_AGENT_ID.to_string(),
            _ => agent_id.clone(),
        };
        let mut memory = Memory {
            slug: slug.clone(),
            agent_id: store_agent_id.clone(),
            title,
            content: content.clone(),
            timestamp: Utc::now(),
            embedding: vec![],
            visibility,
            shared_to,
        };

        let embedding = embedder.embed_text(&content.clone()).await?;
        memory.embedding = embedding;

        let _: Option<Memory> = self
            .conn
            .upsert(("memory", format!("{}/{}", store_agent_id, slug)))
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

        let query = embedder.embed_text(&query).await?;

        let distance_function = DistanceFunction::Cosine;

        let mut response = self
            .conn
            .query(format!(
                r#"SELECT * 
                    FROM type::table($table) 
                    WHERE {distance_function}($query, embedding) >= $threshold 
                        AND (
                            visibility = 'private' AND agent_id = $agent_id
                            OR visibility = 'global'
                            OR (visibility = 'shared' AND array::contains(shared_to, $agent_id))
                        )
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
            .query(
                r#"SELECT * FROM type::table(memory) 
                    WHERE visibility = 'private' AND agent_id = $agent_id
                    OR visibility = 'global'
                    OR (visibility = 'shared' AND array::contains(shared_to, $agent_id))"#,
            )
            .bind(("agent_id", agent_id))
            .await?;

        let data: Vec<Memory> = response.take(0).unwrap();

        Ok(data)
    }

    async fn get_memory_detail(&self, agent_id: String, slug: String) -> Result<Option<Memory>> {
        let mut response = self
            .conn
            .query(
                r#"SELECT * FROM type::table(memory) 
                    WHERE slug = $slug 
                        AND (
                            visibility = 'private' AND agent_id = $agent_id
                            OR visibility = 'global'
                            OR (visibility = 'shared' AND array::contains(shared_to, $agent_id))
                        )"#,
            )
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
