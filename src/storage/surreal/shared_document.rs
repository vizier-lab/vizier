use anyhow::Result;
use chrono::Utc;

use crate::{
    embedding::VizierEmbeddingModel,
    error::VizierError,
    schema::{SharedDocument, SharedDocumentSummary},
    storage::{
        shared_document::SharedDocumentStorage,
        surreal::{DistanceFunction, SurrealStorage},
    },
};

use slugify::slugify;

#[async_trait::async_trait]
impl SharedDocumentStorage for SurrealStorage {
    async fn write_shared_document(
        &self,
        author_agent_id: String,
        slug: Option<String>,
        title: String,
        content: String,
    ) -> Result<()> {
        let embedder = self
            .embedder
            .clone()
            .ok_or(VizierError("embedder is not set".into()))?;

        let slug = slug.unwrap_or_else(|| slugify!(&title));
        let mut doc = SharedDocument {
            slug: slug.clone(),
            author_agent_id: author_agent_id.clone(),
            title,
            content: content.clone(),
            timestamp: Utc::now(),
            embedding: vec![],
        };

        let embedding = embedder.embed_text(&content.clone()).await?;
        doc.embedding = embedding;

        let _: Option<SharedDocument> = self
            .conn
            .upsert(("shared_document", slug))
            .content(doc)
            .await?;

        Ok(())
    }

    async fn query_shared_documents(
        &self,
        query: String,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<SharedDocument>> {
        let embedder = self
            .embedder
            .clone()
            .ok_or(VizierError("embedder is not set".into()))?;

        let query_embedding = embedder.embed_text(&query).await?;

        let distance_function = DistanceFunction::Cosine;

        let mut response = self
            .conn
            .query(format!(
                r#"SELECT * 
                    FROM type::table($table) 
                    WHERE {distance_function}($query, embedding) >= $threshold
                    ORDER BY vector::distance::cosine(embedding, $query) ASC 
                    LIMIT $limit"#
            ))
            .bind(("table", "shared_document"))
            .bind(("query", query_embedding))
            .bind(("limit", limit))
            .bind(("threshold", threshold))
            .await?;

        let res: Vec<SharedDocument> = response.take(0)?;

        Ok(res)
    }

    async fn get_shared_document(&self, slug: String) -> Result<Option<SharedDocument>> {
        let mut response = self
            .conn
            .query("SELECT * FROM type::table(shared_document) WHERE slug = $slug")
            .bind(("slug", slug))
            .await?;

        let data: Option<SharedDocument> = response.take(0)?;

        Ok(data)
    }

    async fn list_shared_documents(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SharedDocumentSummary>> {
        let mut response = self
            .conn
            .query(
                r#"SELECT slug, title, author_agent_id, timestamp 
                   FROM type::table(shared_document)
                   ORDER BY timestamp DESC
                   LIMIT $limit
                   START $offset"#,
            )
            .bind(("offset", offset))
            .bind(("limit", limit))
            .await?;

        let data: Vec<SharedDocumentSummary> = response.take(0)?;

        Ok(data)
    }

    async fn delete_shared_document(&self, author_agent_id: String, slug: String) -> Result<()> {
        let mut response = self
            .conn
            .query("SELECT * FROM type::table(shared_document) WHERE slug = $slug")
            .bind(("slug", slug.clone()))
            .await?;

        let doc: Option<SharedDocument> = response.take(0)?;

        if let Some(doc) = doc {
            if doc.author_agent_id != author_agent_id {
                return Err(anyhow::anyhow!("not authorized to delete this document"));
            }
        }

        let _: Option<SharedDocument> = self.conn.delete(("shared_document", slug)).await?;

        Ok(())
    }
}

