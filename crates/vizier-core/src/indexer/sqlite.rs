use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension};

use crate::{
    embedding::VizierEmbeddingModel,
    indexer::DocumentIndexer,
    schema::DocumentIndex,
};

/// The table the first embedding model in a workspace used, kept by name so an existing install
/// keeps its vectors.
const LEGACY_TABLE: &str = "document_index";

/// The dimension declared by an existing table, if there is one.
fn declared_dimension(conn: &Connection, table: &str) -> Result<Option<usize>> {
    let ddl: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
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

/// The table vectors of `model_dim` are written to: the original `document_index` when it is that
/// wide already (an existing install keeps its index), and one table per other width after that.
///
/// A `vec0` table fixes its width, so two agents whose embedding models differ cannot share one.
/// They used to refuse to start instead; each width gets its own table, so a workspace can host
/// models of any sizes side by side, and a model change reindexes (supervisor).
fn table_for(conn: &Connection, model_dim: usize) -> Result<String> {
    Ok(match declared_dimension(conn, LEGACY_TABLE)? {
        Some(index_dim) if index_dim == model_dim => LEGACY_TABLE.to_string(),
        _ => format!("{LEGACY_TABLE}_d{model_dim}"),
    })
}

/// Writing vectors of the wrong size would silently corrupt recall, so a table of another width is
/// refused with both sizes and the fix (FR-023). With `table_for` this can only happen to a table
/// someone created outside vizier.
fn check_dimension(conn: &Connection, table: &str, model_dim: usize) -> Result<()> {
    if let Some(index_dim) = declared_dimension(conn, table)?
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
    /// The `vec0` table this agent's model writes to, one per vector width.
    table: String,
    dimension: usize,
}

impl SqliteIndexer {
    pub async fn new(
        conn: Arc<Mutex<Connection>>,
        embedder: Arc<crate::embedding::VizierEmbedder>,
    ) -> Result<Self> {
        let test_embedding = embedder.embed_text("test").await?;
        let dim = test_embedding.len();

        let table = {
            let conn = conn.lock();
            let table = table_for(&conn, dim)?;
            check_dimension(&conn, &table, dim)?;
            conn.execute_batch(&format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS {table} USING vec0(
                    embedding float[{}] distance_metric=cosine,
                    context TEXT,
                    path TEXT
                )",
                dim
            ))?;
            table
        };

        Ok(Self {
            conn,
            embedder,
            table,
            dimension: dim,
        })
    }

    /// The vector width this indexer writes.
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// True when this width is the workspace's original `document_index` table, the one an install
    /// upgrading from the single-index layout already has its vectors in.
    pub fn keeps_legacy_table(&self) -> bool {
        self.table == LEGACY_TABLE
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
            &format!("DELETE FROM {} WHERE context = ?1 AND path = ?2", self.table),
            rusqlite::params![context, path],
        )?;
        conn.execute(
            &format!(
                "INSERT INTO {} (embedding, context, path) VALUES (?1, ?2, ?3)",
                self.table
            ),
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
                &format!("DELETE FROM {} WHERE context = ?1 AND path = ?2", self.table),
                rusqlite::params![context, path],
            )?;
            tx.execute(
                &format!(
                    "INSERT INTO {} (embedding, context, path) VALUES (?1, ?2, ?3)",
                    self.table
                ),
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
        let mut stmt = conn.prepare(&format!(
            "SELECT path, context, distance FROM {} WHERE embedding MATCH ?1 AND context = ?2 ORDER BY distance LIMIT ?3",
            self.table
        ))?;

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
            &format!("DELETE FROM {} WHERE context = ?1 AND path = ?2", self.table),
            rusqlite::params![context, path],
        )?;
        Ok(())
    }

    fn dimension(&self) -> Option<usize> {
        Some(self.dimension)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection(dir: &tempfile::TempDir) -> Connection {
        crate::storage::sqlite::SqliteStorage::open_connection(dir.path().to_str().unwrap())
            .unwrap()
    }

    #[test]
    fn a_fresh_workspace_names_the_index_by_dimension() {
        let dir = tempfile::tempdir().unwrap();
        let conn = connection(&dir);

        assert_eq!(table_for(&conn, 384).unwrap(), "document_index_d384");
        assert_eq!(table_for(&conn, 768).unwrap(), "document_index_d768");
    }

    #[test]
    fn an_existing_index_is_reused_for_its_own_dimension_and_keeps_its_name() {
        let dir = tempfile::tempdir().unwrap();
        let conn = connection(&dir);
        conn.execute_batch(
            "CREATE VIRTUAL TABLE document_index USING vec0(
                embedding float[768] distance_metric=cosine,
                context TEXT,
                path TEXT
            )",
        )
        .unwrap();

        // The install that already has vectors keeps them.
        assert_eq!(table_for(&conn, 768).unwrap(), "document_index");
        assert_eq!(declared_dimension(&conn, "document_index").unwrap(), Some(768));
        // Another model gets a table of its own rather than a refusal.
        assert_eq!(table_for(&conn, 1024).unwrap(), "document_index_d1024");
        assert_eq!(table_for(&conn, 384).unwrap(), "document_index_d384");
    }

    #[test]
    fn a_table_of_another_width_is_still_refused_for_that_width() {
        let dir = tempfile::tempdir().unwrap();
        let conn = connection(&dir);
        conn.execute_batch(
            "CREATE VIRTUAL TABLE document_index_d1024 USING vec0(
                embedding float[768] distance_metric=cosine,
                context TEXT,
                path TEXT
            )",
        )
        .unwrap();

        let err = check_dimension(&conn, "document_index_d1024", 1024)
            .unwrap_err()
            .to_string();
        assert!(err.contains("768"), "{err}");
        assert!(err.contains("1024"), "{err}");
        assert!(err.contains("reindexed"), "{err}");
        check_dimension(&conn, "document_index_d1024", 768).unwrap();
    }
}
