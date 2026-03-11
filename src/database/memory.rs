use anyhow::{Ok, Result};
use chrono::Utc;
use rig::embeddings::EmbeddingModel;

use crate::{
    database::{DistanceFunction, VizierDatabases},
    embedding,
    schema::Memory,
};

use slugify::slugify;

impl VizierDatabases {
    pub async fn write_memory(
        &self,
        embedder: &embedding::EmbeddingModel,
        agent_id: String,
        slug: Option<String>,
        title: String,
        content: String,
    ) -> Result<()> {
        let slug = slug.unwrap_or_else(|| slugify!(&title));
        let mut memory = Memory {
            slug: slug.clone(),
            agent_id,
            title,
            content: content.clone(),
            timestamp: Utc::now(),
            embedding: vec![],
        };

        let embedding = embedder.embed_text(&content.clone()).await?;
        memory.embedding = embedding.vec;

        let _: Option<Memory> = self
            .conn
            .upsert(("memory", slug.to_string()))
            .content(memory)
            .await?;

        Ok(())
    }

    pub async fn query_memory(
        &self,
        embedder: &embedding::EmbeddingModel,
        agent_id: String,
        query: String,
        distance_function: DistanceFunction,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<Memory>> {
        let query = embedder.embed_text(&query).await?.vec;

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
}
