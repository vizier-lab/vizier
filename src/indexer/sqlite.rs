use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::Connection;

use crate::{
    embedding::VizierEmbeddingModel,
    indexer::DocumentIndexer,
    schema::DocumentIndex,
};

pub struct SqliteIndexer {
    conn: Arc<Mutex<Connection>>,
    embedder: Arc<crate::embedding::VizierEmbedder>,
}

impl SqliteIndexer {
    pub async fn new(
        conn: Arc<Mutex<Connection>>,
        embedder: Arc<crate::embedding::VizierEmbedder>,
    ) -> Result<Self> {
        let test_embedding = embedder.embed_text("test").await?;
        let dim = test_embedding.len();

        {
            let conn = conn.lock();
            conn.execute_batch(&format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS document_index USING vec0(
                    embedding float[{}] distance_metric=cosine,
                    context TEXT,
                    path TEXT
                )",
                dim
            ))?;
        }

        Ok(Self { conn, embedder })
    }

    fn embedding_to_bytes(embedding: &[f64]) -> Vec<u8> {
        let f32_vals: Vec<f32> = embedding.iter().map(|&v| v as f32).collect();
        let mut bytes = Vec::with_capacity(f32_vals.len() * 4);
        for val in &f32_vals {
            bytes.extend_from_slice(&val.to_le_bytes());
        }
        bytes
    }
}

#[async_trait::async_trait]
impl DocumentIndexer for SqliteIndexer {
    async fn add_document_index(
        &self,
        context: String,
        path: String,
        content: String,
    ) -> Result<DocumentIndex> {
        let embedding = self.embedder.embed_text(&content).await?;
        let embedding_bytes = Self::embedding_to_bytes(&embedding);

        let doc = DocumentIndex {
            path: path.clone(),
            embedding,
            context: context.clone(),
        };

        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM document_index WHERE context = ?1 AND path = ?2",
            rusqlite::params![context, path],
        )?;
        conn.execute(
            "INSERT INTO document_index (embedding, context, path) VALUES (?1, ?2, ?3)",
            rusqlite::params![embedding_bytes, context, path],
        )?;

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
        let query_bytes = Self::embedding_to_bytes(&query_embedding);

        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT path, context, distance FROM document_index WHERE embedding MATCH ?1 AND context = ?2 ORDER BY distance LIMIT ?3",
        )?;

        let results: Vec<DocumentIndex> = stmt
            .query_map(
                rusqlite::params![query_bytes, context, limit as i64],
                |row| {
                    let path: String = row.get(0)?;
                    let ctx: String = row.get(1)?;
                    let distance: f64 = row.get(2)?;
                    Ok((path, ctx, distance))
                },
            )?
            .filter_map(|r| r.ok())
            .filter(|(_, _, distance)| {
                let similarity = 1.0 - distance;
                similarity >= threshold
            })
            .map(|(path, ctx, _)| DocumentIndex {
                path,
                embedding: vec![],
                context: ctx,
            })
            .collect();

        Ok(results)
    }

    async fn delete_index(&self, context: String, path: String) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM document_index WHERE context = ?1 AND path = ?2",
            rusqlite::params![context, path],
        )?;
        Ok(())
    }
}
