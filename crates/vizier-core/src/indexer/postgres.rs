//! Passage vectors on PostgreSQL with the `vector` extension (spec 045, research R6): one table
//! per vector width (`document_index_d<N>`), cosine distance through an HNSW index. Vectors cross
//! the wire as the extension's text form (`'[x,y,…]'`), so no client-side vector type is needed.

use std::sync::Arc;

use anyhow::Result;

use crate::{
    embedding::VizierEmbeddingModel,
    indexer::DocumentIndexer,
    schema::DocumentIndex,
    storage::sql::{Connection, OptionalExtension, SqlStore, params},
};

/// A vector parameter: bound as TEXT and cast, so PostgreSQL infers `text` for the placeholder.
const VECTOR_PARAM: &str = "CAST(CAST(?1 AS TEXT) AS vector)";

pub struct PgIndexer {
    conn: SqlStore,
    embedder: Arc<crate::embedding::VizierEmbedder>,
    table: String,
    dimension: usize,
}

/// The width declared by an existing table's `embedding` column (`vector(N)`), if the table exists.
fn declared_dimension(conn: &Connection, table: &str) -> Result<Option<usize>> {
    let declared: Option<String> = conn
        .query_row(
            "SELECT format_type(a.atttypid, a.atttypmod)
             FROM pg_attribute a
             WHERE a.attrelid = to_regclass(?1) AND a.attname = 'embedding' AND NOT a.attisdropped",
            params![table],
            |row| row.get(0),
        )
        .optional()?;
    let Some(declared) = declared else {
        return Ok(None);
    };
    let width = declared
        .strip_prefix("vector(")
        .and_then(|rest| rest.strip_suffix(')'))
        .and_then(|n| n.trim().parse().ok())
        .ok_or_else(|| anyhow::anyhow!("cannot read the dimension of the existing memory index"))?;
    Ok(Some(width))
}

pub(crate) fn vector_literal(embedding: &[f64]) -> String {
    let mut out = String::with_capacity(embedding.len() * 10 + 2);
    out.push('[');
    for (i, v) in embedding.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&(*v as f32).to_string());
    }
    out.push(']');
    out
}

/// Create (if needed) the vector table for `dimension` and return its name. Refuses a table of
/// another width with the same message as SQLite.
pub(crate) fn ensure_table(conn: &Connection, dimension: usize) -> Result<String> {
    let table = format!("document_index_d{dimension}");
    let tx = conn.transaction()?;
    // Two agents with the same width can start together; serialise the DDL.
    tx.query_row("SELECT pg_advisory_xact_lock(hashtext('vizier_document_index'))", [], |_| Ok(()))?;
    if let Some(found) = declared_dimension(&tx, &table)?
        && found != dimension
    {
        return Err(anyhow::anyhow!(
            "the memory index has dimension {found}, but this embedding model returns \
             {dimension}; the memory index must be rebuilt/reindexed before this embedding \
             model can be used"
        ));
    }
    tx.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {table} (
            context TEXT NOT NULL,
            path TEXT NOT NULL,
            embedding vector({dimension}) NOT NULL
        );
        CREATE INDEX IF NOT EXISTS {table}_doc ON {table} (context, path);
        CREATE INDEX IF NOT EXISTS {table}_hnsw ON {table} USING hnsw (embedding vector_cosine_ops);"
    ))?;
    tx.commit()?;
    Ok(table)
}

/// The `INSERT` for one vector into `table`: `?1` the vector text, `?2` context, `?3` path.
pub(crate) fn insert_sql(table: &str) -> String {
    format!("INSERT INTO {table} (embedding, context, path) VALUES ({VECTOR_PARAM}, ?2, ?3)")
}

impl PgIndexer {
    pub async fn new(conn: SqlStore, embedder: Arc<crate::embedding::VizierEmbedder>) -> Result<Self> {
        let dimension = embedder.embed_text("test").await?.len();
        let table = ensure_table(&conn.lock()?, dimension)?;
        Ok(Self { conn, embedder, table, dimension })
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }

    fn replace(&self, conn: &Connection, context: &str, path: &str, embedding: &[f64]) -> Result<()> {
        conn.execute(
            &format!("DELETE FROM {} WHERE context = ?1 AND path = ?2", self.table),
            params![context, path],
        )?;
        conn.execute(&insert_sql(&self.table), params![vector_literal(embedding), context, path])?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl DocumentIndexer for PgIndexer {
    async fn add_document_index(&self, context: String, path: String, content: String) -> Result<DocumentIndex> {
        let embedding = self.embedder.embed_text(&content).await?;
        self.replace(&self.conn.lock()?, &context, &path, &embedding)?;
        Ok(DocumentIndex { path, embedding, context, score: 0.0 })
    }

    async fn add_document_indexes(
        &self,
        context: String,
        entries: Vec<(String, String)>,
    ) -> Result<Vec<DocumentIndex>> {
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        let contents: Vec<String> = entries.iter().map(|(_, c)| c.clone()).collect();
        let embeddings = self.embedder.embed_texts(contents).await?;
        if embeddings.len() != entries.len() {
            return Err(anyhow::anyhow!(
                "embedder returned {} vectors for {} documents",
                embeddings.len(),
                entries.len()
            ));
        }
        {
            let conn = self.conn.lock()?;
            let tx = conn.transaction()?;
            for ((path, _), embedding) in entries.iter().zip(&embeddings) {
                self.replace(&tx, &context, path, embedding)?;
            }
            tx.commit()?;
        }
        Ok(entries
            .into_iter()
            .zip(embeddings)
            .map(|((path, _), embedding)| DocumentIndex { path, embedding, context: context.clone(), score: 0.0 })
            .collect())
    }

    async fn search_document_index(
        &self,
        context: String,
        query: String,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<DocumentIndex>> {
        let query = vector_literal(&self.embedder.embed_text(&query).await?);
        let conn = self.conn.lock()?;
        let rows = conn
            .prepare(&format!(
                "SELECT path, context, 1 - (embedding <=> {VECTOR_PARAM}) AS score
                 FROM {} WHERE context = ?2
                 ORDER BY embedding <=> {VECTOR_PARAM} LIMIT ?3",
                self.table
            ))?
            .query_map(params![query, context, limit as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, f64>(2)?))
            })?
            .collect::<crate::storage::sql::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .filter(|(_, _, score)| *score >= threshold)
            .map(|(path, context, score)| DocumentIndex { path, embedding: vec![], context, score })
            .collect())
    }

    async fn delete_index(&self, context: String, path: String) -> Result<()> {
        self.conn.lock()?.execute(
            &format!("DELETE FROM {} WHERE context = ?1 AND path = ?2", self.table),
            params![context, path],
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

    #[test]
    fn vectors_are_written_in_the_extension_text_form() {
        assert_eq!(vector_literal(&[0.5, -1.0, 0.25]), "[0.5,-1,0.25]");
        assert_eq!(vector_literal(&[]), "[]");
    }
}
