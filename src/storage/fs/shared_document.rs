use std::path::PathBuf;

use anyhow::{Ok, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::{
    schema::{SharedDocument, SharedDocumentSummary},
    storage::{
        fs::FileSystemStorage, indexer::DocumentIndexer, shared_document::SharedDocumentStorage,
    },
    utils::{self, build_glob_path, build_path},
};

const SHARED_DOCUMENT_PATH: &str = "shared_documents";

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SharedDocumentFrontMatter {
    pub slug: String,
    pub title: String,
    pub author_agent_id: String,
    pub timestamp: chrono::DateTime<Utc>,
}

impl FileSystemStorage {
    pub async fn reindex_shared_documents(&self) -> Result<()> {
        tracing::info!("reindex existing shared documents");
        let base_path = build_path(&self.workspace, &[SHARED_DOCUMENT_PATH]);
        if !base_path.exists() {
            std::fs::create_dir_all(&base_path)?;
        }
        let path = build_glob_path(&self.workspace, &[SHARED_DOCUMENT_PATH, "*.md"]);
        for entry in glob::glob(&path)? {
            let entry = entry?;

            if !entry.is_file() {
                continue;
            }

            self.indices
                .add_document_index(
                    "shared_document".into(),
                    entry.to_str().unwrap().to_string(),
                )
                .await?;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl SharedDocumentStorage for FileSystemStorage {
    async fn write_shared_document(
        &self,
        author_agent_id: String,
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
        let path_str = format!("{}/{}", self.workspace, SHARED_DOCUMENT_PATH);
        let mut path = PathBuf::from(path_str.clone());

        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }

        path.push(format!("{}", slug));

        utils::markdown::write_markdown(
            &SharedDocumentFrontMatter {
                slug: slug.clone(),
                title,
                author_agent_id,
                timestamp: Utc::now(),
            },
            content.clone(),
            path.clone(),
        )?;

        self.indices
            .add_document_index("shared_document".into(), path.to_string_lossy().to_string())
            .await?;

        Ok(())
    }

    async fn query_shared_documents(
        &self,
        query: String,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<SharedDocument>> {
        let documents = self
            .indices
            .search_document_index("shared_document".into(), query, limit, threshold)
            .await?;

        let mut res = vec![];
        for index in documents.iter() {
            tracing::debug!("{:?}", index);
            let (frontmatter, content) = utils::markdown::read_markdown::<SharedDocumentFrontMatter>(
                PathBuf::from(index.path.clone()),
            )?;

            res.push(SharedDocument {
                slug: frontmatter.slug,
                author_agent_id: frontmatter.author_agent_id,
                content,
                title: frontmatter.title,
                timestamp: frontmatter.timestamp,
                embedding: index.embedding.clone(),
            });
        }

        Ok(res)
    }

    async fn get_shared_document(&self, slug: String) -> Result<Option<SharedDocument>> {
        let slug = if slug.ends_with(".md") {
            slug
        } else {
            format!("{}.md", slug)
        };

        let path = format!("{}/{}/{}", self.workspace, SHARED_DOCUMENT_PATH, slug);

        let (frontmatter, content) =
            utils::markdown::read_markdown::<SharedDocumentFrontMatter>(PathBuf::from(path))?;

        let res = SharedDocument {
            slug: frontmatter.slug,
            author_agent_id: frontmatter.author_agent_id,
            content,
            title: frontmatter.title,
            timestamp: frontmatter.timestamp,
            embedding: vec![],
        };

        Ok(Some(res))
    }

    async fn list_shared_documents(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SharedDocumentSummary>> {
        let path = build_glob_path(&self.workspace, &[SHARED_DOCUMENT_PATH, "*"]);

        let mut res = vec![];
        for entry in glob::glob(&path)? {
            let entry = entry?;

            if !entry.is_file() {
                continue;
            }

            let (frontmatter, _) =
                utils::markdown::read_markdown::<SharedDocumentFrontMatter>(entry)?;

            res.push(SharedDocumentSummary {
                slug: frontmatter.slug,
                title: frontmatter.title,
                author_agent_id: frontmatter.author_agent_id,
                timestamp: frontmatter.timestamp,
            });
        }

        res.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        let end = std::cmp::min(offset + limit, res.len());
        if offset >= res.len() {
            return Ok(vec![]);
        }

        Ok(res[offset..end].to_vec())
    }

    async fn delete_shared_document(&self, author_agent_id: String, slug: String) -> Result<()> {
        let slug = if slug.ends_with(".md") {
            slug
        } else {
            format!("{}.md", slug)
        };

        let path = format!("{}/{}/{}", self.workspace, SHARED_DOCUMENT_PATH, slug);

        let (frontmatter, _) =
            utils::markdown::read_markdown::<SharedDocumentFrontMatter>(PathBuf::from(&path))?;

        if frontmatter.author_agent_id != author_agent_id {
            return Err(anyhow::anyhow!("not authorized to delete this document"));
        }

        self.indices
            .delete_index("shared_document".into(), path.clone())
            .await?;
        std::fs::remove_file(path)?;

        Ok(())
    }
}

