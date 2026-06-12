use std::sync::Arc;

use anyhow::Result;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

use crate::{
    embedding::VizierEmbeddingModel,
    schema::DocumentIndex,
    storage::surreal::DistanceFunction,
};

pub struct SurrealIndexer {
    conn: Arc<Surreal<Db>>,
    embedder: Arc<crate::embedding::VizierEmbedder>,
}

impl SurrealIndexer {
    pub fn new(conn: Arc<Surreal<Db>>, embedder: Arc<crate::embedding::VizierEmbedder>) -> Self {
        Self { conn, embedder }
    }
}

#[async_trait::async_trait]
impl crate::indexer::DocumentIndexer for SurrealIndexer {
    async fn add_document_index(
        &self,
        context: String,
        path: String,
        content: String,
    ) -> Result<DocumentIndex> {
        let embedding = self.embedder.embed_text(&content).await?;
        let doc = DocumentIndex {
            path: path.clone(),
            embedding,
            context: context.clone(),
        };

        let key = format!("{}#{}", context, path);
        let _: Option<DocumentIndex> = self
            .conn
            .upsert(("document_index", key))
            .content(doc.clone())
            .await?;

        Ok(doc)
    }

    async fn search_document_index(
        &self,
        context: String,
        query: String,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<DocumentIndex>> {
        let query_embedding = self.embedder.embed_text(&query).await?;
        let distance_function = DistanceFunction::Cosine;

        let mut response = self
            .conn
            .query(format!(
                r#"SELECT *
                    FROM type::table($table)
                    WHERE {distance_function}($query, embedding) >= $threshold AND context = $context
                    ORDER BY distance ASC
                    LIMIT $limit"#
            ))
            .bind(("table", "document_index"))
            .bind(("query", query_embedding))
            .bind(("limit", limit))
            .bind(("threshold", threshold))
            .bind(("context", context))
            .await?;

        let res: Vec<DocumentIndex> = response.take(0)?;

        Ok(res)
    }

    async fn delete_index(&self, context: String, path: String) -> Result<()> {
        let key = format!("{}#{}", context, path);
        let _: Option<DocumentIndex> = self.conn.delete(("document_index", key)).await?;
        Ok(())
    }
}
