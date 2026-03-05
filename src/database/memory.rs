use anyhow::{Ok, Result};
use chrono::Utc;
use rig::embeddings::EmbeddingModel;

use crate::{
    database::{DistanceFunction, VizierDatabases, schema::Memory},
    embedding,
};

use slugify::slugify;

impl VizierDatabases {
    pub async fn write_memory(
        &self,
        embedder: &embedding::EmbeddingModel,
        slug: Option<String>,
        title: String,
        content: String,
    ) -> Result<()> {
        let slug = slug.unwrap_or_else(|| slugify!(&title));
        let mut memory = Memory {
            slug: slug.clone(),
            title,
            content: content.clone(),
            timestamp: Utc::now(),
            embedding: vec![],
        };

        let embedding = embedder.embed_text(&content.clone()).await?;
        memory.embedding = embedding.vec;

        let _: Option<Memory> = self.conn.upsert(("memory", &slug)).content(memory).await?;

        Ok(())
    }

    pub async fn query_memory(
        &self,
        embedder: &embedding::EmbeddingModel,
        query: String,
        distance_function: DistanceFunction,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<Memory>> {
        let query = embedder.embed_text(&query).await?.vec;
        println!("query: {}", query.len());

        let mut response = self
            .conn
            .query(format!(
                r#"SELECT * 
                    FROM type::table($table) 
                    WHERE {distance_function}($query, embedding) >= $threshold 
                    ORDER BY distance ASC 
                    LIMIT $limit"#
            ))
            .bind(("table", "memory"))
            .bind(("query", query))
            .bind(("limit", limit))
            .bind(("threshold", threshold))
            .await?;

        let res: Vec<Memory> = response.take(0)?;

        Ok(res)
    }
}
