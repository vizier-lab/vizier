use std::{path::PathBuf, str::FromStr};

use anyhow::Result;

use crate::{
    embedding::VizierEmbeddingModel,
    error::VizierError,
    schema::DocumentIndex,
    storage::{
        indexer::DocumentIndexer,
        surreal::{DistanceFunction, SurrealStorage},
    },
};

#[async_trait::async_trait]
impl DocumentIndexer for SurrealStorage {
    async fn add_document_index(&self, context: String, path: String) -> Result<DocumentIndex> {
        let path_buf = PathBuf::from_str(&path)?;

        let content = crate::utils::markdown::read_content(path_buf)?;
        let embedder = self.embedder.clone();
        let embedding = embedder
            .ok_or(VizierError("embedder not available".into()))?
            .embed_text(&content)
            .await?;

        let doc = DocumentIndex {
            path: path.clone(),
            embedding,
            context: context.clone(),
        };

        let key = format!("{}#{}", context, path.clone());
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
                    WHERE {distance_function}($query, embedding) >= $threshold AND context = $context
                    ORDER BY distance ASC 
                    LIMIT $limit"#
            ))
            .bind(("table", "document_index"))
            .bind(("query", query))
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
