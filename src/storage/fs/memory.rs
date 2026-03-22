use std::path::PathBuf;

use anyhow::{Ok, Result};
use chrono::Utc;
use rig::embeddings::EmbeddingModel;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::{
    error::VizierError,
    schema::{DocumentIndex, Memory},
    storage::{
        fs::{FileSystemStorage, MEMORY_PATH},
        memory::MemoryStorage,
    },
    utils,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MemoryFrontMatter {
    pub slug: String,
    pub title: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub agent_id: String,
}

impl From<Memory> for MemoryFrontMatter {
    fn from(value: Memory) -> Self {
        Self {
            slug: value.slug,
            title: value.title,
            timestamp: value.timestamp,
            agent_id: value.agent_id,
        }
    }
}

impl FileSystemStorage {
    pub async fn reindex_memory(&self) -> Result<()> {
        if self.embedder.is_none() {
            return Ok(());
        }

        log::info!("reindex existing memory");
        let path = format!("{}/agents/**/{MEMORY_PATH}/*.md", self.workspace);
        for entry in glob::glob(&path)? {
            let entry = entry?;

            if !entry.is_file() {
                continue;
            }

            log::info!("reindex {:?}", entry);
            let (frontmatter, content) =
                utils::markdown::read_markdown::<MemoryFrontMatter>(entry)?;

            let embedding = self
                .embedder
                .clone()
                .unwrap()
                .embed_text(&content)
                .await?
                .vec;
            let memory = Memory {
                slug: frontmatter.slug,
                agent_id: frontmatter.agent_id,
                content,
                title: frontmatter.title,
                timestamp: frontmatter.timestamp,
                embedding: embedding.clone(),
            };

            let path_str = format!(
                "{}/agents/{}/{}/{}",
                self.workspace,
                memory.agent_id.clone(),
                MEMORY_PATH,
                memory.slug.clone()
            );
            self.memory_indices.lock().await.insert(
                path_str.clone(),
                DocumentIndex {
                    path: path_str,
                    embedding,
                },
            );
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl MemoryStorage for FileSystemStorage {
    async fn write_memory(
        &self,
        agent_id: String,
        slug: Option<String>,
        title: String,
        content: String,
    ) -> Result<()> {
        let embedder = self
            .embedder
            .clone()
            .ok_or(VizierError("embedder is not set".into()))?;

        let slug = slug.unwrap_or_else(|| slugify!(&title));
        let slug = if slug.ends_with(".md") {
            slug
        } else {
            format!("{}.md", slug)
        };
        let path_str = format!(
            "{}/agents/{}/{}",
            self.workspace,
            agent_id.clone(),
            MEMORY_PATH
        );
        let mut path = PathBuf::from(path_str.clone());

        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }

        path.push(format!("{}", slug));

        utils::markdown::write_markdown(
            &MemoryFrontMatter {
                slug,
                title,
                timestamp: Utc::now(),
                agent_id,
            },
            content.clone(),
            path.clone(),
        )?;

        let embedding = embedder.embed_text(&content).await?.vec;
        self.memory_indices.lock().await.insert(
            path_str.clone(),
            DocumentIndex {
                path: path_str,
                embedding,
            },
        );

        Ok(())
    }

    async fn query_memory(
        &self,
        agent_id: String,
        query: String,
        limit: usize,
        _threshold: f64,
    ) -> Result<Vec<Memory>> {
        let embedder = self
            .embedder
            .clone()
            .ok_or(VizierError("embedder is not set".into()))?;

        let q_embedding = embedder.embed_text(&query).await?.vec;

        let path = format!(
            "{}/agents/{}/{}",
            self.workspace,
            agent_id.clone(),
            MEMORY_PATH
        );

        let indices = self.memory_indices.lock().await;

        let mut documents = indices
            .iter()
            .filter(|index| index.0.contains(&path))
            .map(|(_, index)| {
                let distance = q_embedding
                    .iter()
                    .zip(&index.embedding)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>()
                    .sqrt();

                (index.clone(), distance)
            })
            .take(limit)
            .collect::<Vec<(DocumentIndex, f64)>>();

        documents.sort_by(|a, b| a.1.total_cmp(&b.1));

        let mut res = vec![];
        for (index, _) in documents.iter() {
            let (frontmatter, content) = utils::markdown::read_markdown::<MemoryFrontMatter>(
                PathBuf::from(index.path.clone()),
            )?;

            res.push(Memory {
                slug: frontmatter.slug,
                agent_id: frontmatter.agent_id,
                content,
                title: frontmatter.title,
                timestamp: frontmatter.timestamp,
                embedding: index.embedding.clone(),
            });
        }

        Ok(res)
    }

    async fn get_all_agent_memory(&self, agent_id: String) -> Result<Vec<Memory>> {
        let path = format!(
            "{}/agents/{}/{}/",
            self.workspace,
            agent_id.clone(),
            MEMORY_PATH
        );

        let indices = self.memory_indices.lock().await;
        let indices = indices
            .iter()
            .filter(|index| index.0.contains(&path))
            .map(|index| index.1.clone())
            .collect::<Vec<_>>();

        let mut res = vec![];
        for index in &indices {
            let (frontmatter, content) =
                utils::markdown::read_markdown::<MemoryFrontMatter>(PathBuf::from(path.clone()))?;

            res.push(Memory {
                slug: frontmatter.slug,
                agent_id: frontmatter.agent_id,
                content,
                title: frontmatter.title,
                timestamp: frontmatter.timestamp,
                embedding: index.embedding.clone(),
            });
        }

        Ok(res)
    }

    async fn get_memory_detail(&self, agent_id: String, slug: String) -> Result<Option<Memory>> {
        let slug = if slug.ends_with(".md") {
            slug
        } else {
            format!("{}.md", slug)
        };

        let path = format!(
            "{}/agents/{}/{}/{}",
            self.workspace,
            agent_id.clone(),
            MEMORY_PATH,
            slug
        );

        let (frontmatter, content) = utils::markdown::read_markdown::<MemoryFrontMatter>(
            PathBuf::from(PathBuf::from(path.clone())),
        )?;

        let indices = self.memory_indices.lock().await;
        let index = indices.get(&path).ok_or(VizierError("not found".into()))?;
        let res = Memory {
            slug: frontmatter.slug,
            agent_id: frontmatter.agent_id,
            content,
            title: frontmatter.title,
            timestamp: frontmatter.timestamp,
            embedding: index.embedding.clone(),
        };

        Ok(Some(res))
    }

    async fn delete_memory(&self, agent_id: String, slug: String) -> Result<()> {
        let slug = if slug.ends_with(".md") {
            slug
        } else {
            format!("{}.md", slug)
        };

        let path = format!(
            "{}/agents/{}/{}/{}",
            self.workspace,
            agent_id.clone(),
            MEMORY_PATH,
            slug
        );

        let mut indices = self.memory_indices.lock().await;
        indices.remove(&path);

        std::fs::remove_file(path)?;

        Ok(())
    }
}
