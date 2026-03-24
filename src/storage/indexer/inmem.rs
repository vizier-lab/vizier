use std::{collections::HashMap, path::PathBuf, str::FromStr, sync::Arc};

use anyhow::Result;
use tokio::sync::Mutex;

use crate::{
    embedding::{VizierEmbedder, VizierEmbeddingModel},
    error::VizierError,
    schema::DocumentIndex,
    storage::indexer::DocumentIndexer,
};

pub struct InMemIndexer {
    index: Arc<Mutex<HashMap<String, DocumentIndex>>>,
    embedder: Option<Arc<VizierEmbedder>>,
}

impl InMemIndexer {
    pub fn new(embedder: Option<Arc<VizierEmbedder>>) -> Self {
        Self {
            index: Arc::new(Mutex::new(HashMap::new())),
            embedder,
        }
    }
}

#[async_trait::async_trait]
impl DocumentIndexer for InMemIndexer {
    async fn add_document_index(&self, context: String, path: String) -> Result<DocumentIndex> {
        let embedder = self
            .embedder
            .clone()
            .ok_or(VizierError("embedder is not set".into()))?;

        let mut index = self.index.lock().await;
        let path_buf = PathBuf::from_str(&path)?;
        let content = crate::utils::markdown::read_content(path_buf)?;

        let embedding = embedder.embed_text(&content).await?;

        let document_index = DocumentIndex {
            path: path.clone(),
            context: context.clone(),
            embedding,
        };

        let key = format!("{}#{}", context, path);
        index.insert(key, document_index.clone());

        Ok(document_index)
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

        let q_embedding = embedder.embed_text(&query).await?;

        let indices = self.index.clone();
        let mut documents = indices
            .lock()
            .await
            .iter()
            .filter(|(_, item)| item.context == context)
            .map(|(_, index)| {
                let distance = q_embedding
                    .iter()
                    .zip(&index.embedding)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>()
                    .sqrt();

                (index.clone(), distance)
            })
            .filter(|(_, distance)| *distance <= 1. - threshold)
            .take(limit)
            .collect::<Vec<(DocumentIndex, f64)>>();

        documents.sort_by(|a, b| a.1.total_cmp(&b.1));

        Ok(documents.iter().map(|(doc, _)| doc.clone()).collect())
    }

    async fn delete_index(&self, context: String, path: String) -> Result<()> {
        let key = format!("{}#{}", context, path);

        let index = self.index.clone();
        index.lock().await.remove(&key);

        Ok(())
    }
}
