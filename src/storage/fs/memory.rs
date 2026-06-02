use std::path::PathBuf;

use anyhow::{Ok, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::{
    schema::{Memory, MemoryVisibility},
    storage::{
        fs::{FileSystemStorage, MEMORY_PATH},
        indexer::DocumentIndexer,
        memory::MemoryStorage,
    },
    utils::{self, build_glob_path, build_path},
};

const GLOBAL_AGENT_ID: &str = "_global";

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MemoryFrontMatter {
    pub slug: String,
    pub title: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub agent_id: String,
    #[serde(default)]
    pub visibility: MemoryVisibility,
    #[serde(default)]
    pub shared_to: Vec<String>,
}

impl From<Memory> for MemoryFrontMatter {
    fn from(value: Memory) -> Self {
        Self {
            slug: value.slug,
            title: value.title,
            timestamp: value.timestamp,
            agent_id: value.agent_id,
            visibility: value.visibility,
            shared_to: value.shared_to,
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

    fn can_access_memory(agent_id: &str, frontmatter: &MemoryFrontMatter) -> bool {
        match frontmatter.visibility {
            MemoryVisibility::Private => frontmatter.agent_id == agent_id,
            MemoryVisibility::Global => true,
            MemoryVisibility::Shared => frontmatter.shared_to.contains(&agent_id.to_string()),
        }
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
        visibility: MemoryVisibility,
        shared_to: Vec<String>,
    ) -> Result<()> {
        let slug = slug.unwrap_or_else(|| slugify!(&title));
        let slug = if slug.ends_with(".md") {
            slug
        } else {
            format!("{}.md", slug)
        };

        let store_agent_id = match visibility {
            MemoryVisibility::Global => GLOBAL_AGENT_ID.to_string(),
            _ => agent_id.clone(),
        };

        let path_str = format!(
            "{}/agents/{}/{}",
            self.workspace, store_agent_id, MEMORY_PATH
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
                agent_id: store_agent_id,
                visibility,
                shared_to,
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
            .search_document_index("memory".into(), query, limit * 3, threshold)
            .await?;

        let mut res = vec![];
        for index in documents.iter() {
            tracing::debug!("{:?}", index);
            let (frontmatter, content) = utils::markdown::read_markdown::<MemoryFrontMatter>(
                PathBuf::from(index.path.clone()),
            )?;

            if !Self::can_access_memory(&agent_id, &frontmatter) {
                continue;
            }

            res.push(Memory {
                slug: frontmatter.slug,
                agent_id: frontmatter.agent_id,
                content,
                title: frontmatter.title,
                timestamp: frontmatter.timestamp,
                embedding: index.embedding.clone(),
                visibility: frontmatter.visibility,
                shared_to: frontmatter.shared_to,
            });

            if res.len() >= limit {
                break;
            }
        }

        Ok(res)
    }

    async fn get_all_agent_memory(&self, agent_id: String) -> Result<Vec<Memory>> {
        let mut res = vec![];

        for agent_dir in [&agent_id, GLOBAL_AGENT_ID] {
            let path = build_glob_path(&self.workspace, &["agents", agent_dir, MEMORY_PATH, "*"]);

            for entry in glob::glob(&path)? {
                let entry = entry?;

                if !entry.is_file() {
                    continue;
                }

                let (frontmatter, content) =
                    utils::markdown::read_markdown::<MemoryFrontMatter>(entry)?;

                if !Self::can_access_memory(&agent_id, &frontmatter) {
                    continue;
                }

                res.push(Memory {
                    slug: frontmatter.slug,
                    agent_id: frontmatter.agent_id,
                    content,
                    title: frontmatter.title,
                    timestamp: frontmatter.timestamp,
                    embedding: vec![],
                    visibility: frontmatter.visibility,
                    shared_to: frontmatter.shared_to,
                });
            }
        }

        Ok(res)
    }

    async fn get_memory_detail(&self, agent_id: String, slug: String) -> Result<Option<Memory>> {
        let slug = if slug.ends_with(".md") {
            slug
        } else {
            format!("{}.md", slug)
        };

        for agent_dir in [&agent_id, GLOBAL_AGENT_ID] {
            let path = format!(
                "{}/agents/{}/{}/{}",
                self.workspace, agent_dir, MEMORY_PATH, slug
            );

            if !PathBuf::from(path.clone()).exists() {
                continue;
            }

            let (frontmatter, content) = utils::markdown::read_markdown::<MemoryFrontMatter>(
                PathBuf::from(path.clone()),
            )?;

            if Self::can_access_memory(&agent_id, &frontmatter) {
                return Ok(Some(Memory {
                    slug: frontmatter.slug,
                    agent_id: frontmatter.agent_id,
                    content,
                    title: frontmatter.title,
                    timestamp: frontmatter.timestamp,
                    embedding: vec![],
                    visibility: frontmatter.visibility,
                    shared_to: frontmatter.shared_to,
                }));
            }
        }

        Ok(None)
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
