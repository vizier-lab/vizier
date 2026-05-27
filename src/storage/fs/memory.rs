use std::path::PathBuf;

use anyhow::{Ok, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::{
    schema::Memory,
    storage::{
        fs::{FileSystemStorage, MEMORY_PATH},
        indexer::DocumentIndexer,
        memory::MemoryStorage,
    },
    utils::{self, build_glob_path, build_path},
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
        tracing::info!("reindex existing memory");
        let base_path = build_path(&self.workspace, &["agents"]);
        if !base_path.exists() {
            std::fs::create_dir_all(&base_path)?;
        }
        let path = build_glob_path(&self.workspace, &["agents", "**", MEMORY_PATH, "*.md"]);
        for entry in glob::glob(&path)? {
            let entry = entry?;

            if !entry.is_file() {
                continue;
            }

            self.indices
                .add_document_index("memory".into(), entry.to_str().unwrap().to_string())
                .await?;
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

        self.indices
            .add_document_index("memory".into(), path_str.clone())
            .await?;

        Ok(())
    }

    async fn query_memory(
        &self,
        agent_id: String,
        query: String,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<Memory>> {
        let documents = self
            .indices
            .search_document_index("memory".into(), query, limit, threshold)
            .await?;

        let mut res = vec![];
        for index in documents.iter() {
            tracing::debug!("{:?}", index);
            let (frontmatter, content) = utils::markdown::read_markdown::<MemoryFrontMatter>(
                PathBuf::from(index.path.clone()),
            )?;

            // TODO: need better handling to check agent_id on index level
            // especially on multi agent workflow
            if frontmatter.agent_id != agent_id {
                continue;
            }

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
        let path = build_glob_path(&self.workspace, &["agents", &agent_id, MEMORY_PATH, "*"]);

        let mut res = vec![];
        for entry in glob::glob(&path)? {
            let entry = entry?;

            if !entry.is_file() {
                continue;
            }

            let (frontmatter, content) =
                utils::markdown::read_markdown::<MemoryFrontMatter>(entry)?;

            res.push(Memory {
                slug: frontmatter.slug,
                agent_id: frontmatter.agent_id,
                content,
                title: frontmatter.title,
                timestamp: frontmatter.timestamp,
                embedding: vec![],
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

        let res = Memory {
            slug: frontmatter.slug,
            agent_id: frontmatter.agent_id,
            content,
            title: frontmatter.title,
            timestamp: frontmatter.timestamp,
            embedding: vec![],
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

        self.indices
            .delete_index("memory".into(), path.clone())
            .await?;
        std::fs::remove_file(path)?;

        Ok(())
    }
}
