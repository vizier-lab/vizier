use std::{collections::HashMap, fs, path::PathBuf, str::FromStr, sync::Arc};

use anyhow::Result;
use rig::embeddings::EmbeddingModel;

use crate::{error::VizierError, schema::DocumentIndex};

pub struct IndexedDocuments {
    pub documents: HashMap<String, DocumentIndex>,
    embedder: Arc<crate::embedding::EmbeddingModel>,
}

impl IndexedDocuments {
    pub async fn new(
        paths: Vec<String>,
        embedder: Arc<crate::embedding::EmbeddingModel>,
    ) -> Result<Self> {
        let mut res = Self {
            documents: HashMap::new(),
            embedder,
        };

        for path in paths {
            res.add(path);
        }

        Ok(res)
    }

    pub async fn add(&mut self, path: String) -> Result<()> {
        // check if document exiss
        let path_buf = PathBuf::from_str(&path.clone())?;
        if !(path_buf.exists() && path_buf.is_file()) {
            return Err(VizierError(format!("file not found {}", path)).into());
        }

        let content = fs::read_to_string(&path_buf)?;

        let embedding = self.embedder.embed_text(&content.clone()).await?.vec;

        self.documents
            .insert(path.clone(), DocumentIndex { path, embedding });

        Ok(())
    }

    pub async fn search_n(&self, query: String, n: usize) -> Result<Vec<DocumentIndex>> {
        let q_embedding = self.embedder.embed_text(&query).await?.vec;

        let mut distances = self
            .documents
            .iter()
            .map(|(_, document)| {
                let distance = q_embedding
                    .iter()
                    .zip(&document.embedding)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>()
                    .sqrt();

                (document, distance)
            })
            .collect::<Vec<_>>();

        distances.sort_by(|a, b| a.1.total_cmp(&b.1));

        Ok(distances.iter().take(n).map(|x| x.0.clone()).collect())
    }
}
