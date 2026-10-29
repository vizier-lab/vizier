use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension};

use crate::{
    embedding::VizierEmbeddingModel,
    indexer::DocumentIndexer,
    schema::DocumentIndex,
};

/// The dimension declared by an existing `document_index` table, if there is one.
fn declared_dimension(conn: &Connection) -> Result<Option<usize>> {
    let ddl: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'document_index'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(ddl) = ddl else {
        return Ok(None);
    };
    let start = ddl
        .find("float[")
        .map(|at| at + "float[".len())
        .ok_or_else(|| anyhow::anyhow!("cannot read the dimension of the existing memory index"))?;
    let end = ddl[start..]
        .find(']')
        .map(|at| at + start)
        .ok_or_else(|| anyhow::anyhow!("cannot read the dimension of the existing memory index"))?;
    Ok(Some(ddl[start..end].trim().parse()?))
}

/// A different embedding model can return vectors of another size. Writing them into an index
/// built for the old size would silently corrupt recall, so the mismatch is refused with both
/// sizes and the fix (FR-023).
fn check_dimension(conn: &Connection, model_dim: usize) -> Result<()> {
    if let Some(index_dim) = declared_dimension(conn)?
        && index_dim != model_dim
    {
        return Err(anyhow::anyhow!(
            "the memory index has dimension {index_dim}, but this embedding model returns \
             {model_dim}; the memory index must be rebuilt/reindexed before this embedding \
             model can be used"
        ));
    }
    Ok(())
}

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
            check_dimension(&conn, dim)?;
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
            score: 0.0,
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

    async fn add_document_indexes(
        &self,
        context: String,
        entries: Vec<(String, String)>,
    ) -> Result<Vec<DocumentIndex>> {
        if entries.is_empty() {
            return Ok(Vec::new());
        }

        // One embedding call for the whole batch; this is the entire point of the method.
        let contents: Vec<String> = entries.iter().map(|(_, c)| c.clone()).collect();
        let embeddings = self.embedder.embed_texts(contents).await?;
        if embeddings.len() != entries.len() {
            return Err(anyhow::anyhow!(
                "embedder returned {} vectors for {} documents",
                embeddings.len(),
                entries.len()
            ));
        }

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        for ((path, _), embedding) in entries.iter().zip(embeddings.iter()) {
            let bytes = Self::embedding_to_bytes(embedding);
            tx.execute(
                "DELETE FROM document_index WHERE context = ?1 AND path = ?2",
                rusqlite::params![context, path],
            )?;
            tx.execute(
                "INSERT INTO document_index (embedding, context, path) VALUES (?1, ?2, ?3)",
                rusqlite::params![bytes, context, path],
            )?;
        }
        tx.commit()?;

        Ok(entries
            .into_iter()
            .zip(embeddings)
            .map(|((path, _), embedding)| DocumentIndex {
                path,
                embedding,
                context: context.clone(),
                score: 0.0,
            })
            .collect())
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
            .filter_map(|(path, ctx, distance)| {
                // The similarity was already being computed here for the threshold filter and
                // then dropped, leaving ranking nothing to rank on (research Decision 4). It is
                // the same value; it now survives the map.
                let score = 1.0 - distance;
                (score >= threshold).then_some(DocumentIndex {
                    path,
                    embedding: vec![],
                    context: ctx,
                    score,
                })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pre_existing_index_dimension_must_match_the_model() {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::storage::sqlite::SqliteStorage::open_connection(
            dir.path().to_str().unwrap(),
        )
        .unwrap();

        // No table yet: the probe's dimension creates it, as before.
        check_dimension(&conn, 1024).unwrap();

        conn.execute_batch(
            "CREATE VIRTUAL TABLE document_index USING vec0(
                embedding float[768] distance_metric=cosine,
                context TEXT,
                path TEXT
            )",
        )
        .unwrap();
        assert_eq!(declared_dimension(&conn).unwrap(), Some(768));
        let err = check_dimension(&conn, 1024).unwrap_err().to_string();
        assert!(err.contains("768"), "{err}");
        assert!(err.contains("1024"), "{err}");
        assert!(err.contains("reindexed"), "{err}");
        check_dimension(&conn, 768).unwrap();
    }
}
