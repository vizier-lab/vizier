use std::path::PathBuf;

use anyhow::{Ok, Result};
use chrono::Utc;
use regex::Regex;
use slugify::slugify;

use crate::{
    indexer::{DocumentIndexer, VizierIndexer},
    schema::{
        Memory, MemoryFrontMatter, MemoryGraph, MemoryGraphEdge, MemoryGraphNode,
        MemoryQueryParams, MemoryVisibility, PaginatedMemory, VizierAttachment,
    },
    storage::{
        fs::{FileSystemStorage, MEMORY_PATH},
        memory::MemoryStorage,
    },
    utils::{self, build_glob_path, build_path},
};

const GLOBAL_AGENT_ID: &str = "_global";

fn parse_wikilinks(content: &str) -> Vec<String> {
    let re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
    re.captures_iter(content)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

impl FileSystemStorage {
    fn can_access_memory(agent_id: &str, frontmatter: &MemoryFrontMatter) -> bool {
        match frontmatter.visibility {
            MemoryVisibility::Private => frontmatter.agent_id == agent_id,
            MemoryVisibility::Global => true,
            MemoryVisibility::Shared => frontmatter.shared_to.contains(&agent_id.to_string()),
        }
    }

    fn load_memory_from_path(
        path: PathBuf,
        agent_id: &str,
    ) -> Result<Option<(MemoryFrontMatter, String)>> {
        if !path.exists() {
            return Ok(None);
        }

        let (frontmatter, content) =
            utils::markdown::read_markdown::<MemoryFrontMatter>(path)?;

        if !Self::can_access_memory(agent_id, &frontmatter) {
            return Ok(None);
        }

        Ok(Some((frontmatter, content)))
    }

    fn memory_from_frontmatter(frontmatter: MemoryFrontMatter, content: String) -> Memory {
        Memory {
            slug: frontmatter.slug,
            agent_id: frontmatter.agent_id,
            content,
            title: frontmatter.title,
            timestamp: frontmatter.timestamp,
            visibility: frontmatter.visibility,
            shared_to: frontmatter.shared_to,
            tags: frontmatter.tags,
            keywords: frontmatter.keywords,
            relations: frontmatter.relations,
            attachments: frontmatter.attachments,
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
        tags: Vec<String>,
        attachments: Vec<VizierAttachment>,
        indexer: &VizierIndexer,
    ) -> Result<Memory> {
        let slug = slug.unwrap_or_else(|| slugify!(&title));

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

        path.push(format!("{}.md", slug));

        let slug_with_ext = format!("{}.md", slug);
        for agent_dir in [&agent_id, GLOBAL_AGENT_ID] {
            let old_path = format!(
                "{}/agents/{}/{}/{}",
                self.workspace, agent_dir, MEMORY_PATH, slug_with_ext
            );
            if old_path != path.to_string_lossy().to_string() {
                let pb = PathBuf::from(&old_path);
                if pb.exists() {
                    let _ = indexer
                        .delete_index("memory".into(), old_path.clone())
                        .await;
                    let _ = std::fs::remove_file(&pb);
                }
            }
        }

        let relations = parse_wikilinks(&content);

        let frontmatter = MemoryFrontMatter {
            slug: slug.clone(),
            title: title.clone(),
            timestamp: Utc::now(),
            agent_id: store_agent_id.clone(),
            visibility: visibility.clone(),
            shared_to: shared_to.clone(),
            tags: tags.clone(),
            keywords: vec![],
            relations: relations.clone(),
            attachments: attachments.clone(),
        };

        utils::markdown::write_markdown(&frontmatter, content.clone(), path.clone())?;

        let path_for_index = path.to_string_lossy().to_string();
        indexer
            .add_document_index("memory".into(), path_for_index, content.clone())
            .await?;

        Ok(Memory {
            slug,
            agent_id: store_agent_id,
            content,
            title,
            timestamp: frontmatter.timestamp,
            visibility,
            shared_to,
            tags,
            keywords: vec![],
            relations,
            attachments,
        })
    }

    async fn query_memory(
        &self,
        agent_id: String,
        query: String,
        limit: usize,
        threshold: f64,
        indexer: &VizierIndexer,
    ) -> Result<Vec<Memory>> {
        let fetch_limit = limit * 5;
        let documents = indexer
            .search_document_index("memory".into(), query, fetch_limit, threshold)
            .await?;

        let mut candidates = vec![];
        for index in documents.iter() {
            let path = PathBuf::from(index.path.clone());
            if let Some((frontmatter, content)) = Self::load_memory_from_path(path, &agent_id)? {
                candidates.push(Self::memory_from_frontmatter(frontmatter, content));
            }
        }

        let all_memories = self.get_all_agent_memory(agent_id).await?;
        let reranked = crate::storage::rerank::rerank_memories(candidates, &all_memories);
        Ok(reranked.into_iter().take(limit).collect())
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

                if let Some((frontmatter, content)) =
                    Self::load_memory_from_path(entry, &agent_id)?
                {
                    res.push(Self::memory_from_frontmatter(frontmatter, content));
                }
            }
        }

        Ok(res)
    }

    async fn get_filtered_memories(
        &self,
        params: MemoryQueryParams,
    ) -> Result<PaginatedMemory> {
        let all_memories = self.get_all_agent_memory(params.agent_id.clone()).await?;

        let mut filtered: Vec<Memory> = all_memories
            .into_iter()
            .filter(|m| {
                if let Some(ref visibility) = params.visibility {
                    if &m.visibility != visibility {
                        return false;
                    }
                }

                if let Some(ref tags) = params.tags {
                    if !tags.is_empty() && !tags.iter().any(|t| m.tags.contains(t)) {
                        return false;
                    }
                }

                true
            })
            .collect();

        let total = filtered.len();

        filtered.sort_by(|a, b| {
            let ord = match params.sort_by.as_deref() {
                Some("title") => a.title.cmp(&b.title),
                Some("slug") => a.slug.cmp(&b.slug),
                _ => b.timestamp.cmp(&a.timestamp),
            };
            if params.sort_order.as_deref() == Some("asc") {
                ord.reverse()
            } else {
                ord
            }
        });

        filtered = filtered
            .into_iter()
            .skip(params.offset)
            .take(params.limit)
            .collect();

        Ok(PaginatedMemory {
            memories: filtered,
            total,
            offset: params.offset,
            limit: params.limit,
        })
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

            if let Some((frontmatter, content)) =
                Self::load_memory_from_path(PathBuf::from(path), &agent_id)?
            {
                return Ok(Some(Self::memory_from_frontmatter(frontmatter, content)));
            }
        }

        Ok(None)
    }

    async fn get_related_memories(
        &self,
        agent_id: String,
        slug: String,
    ) -> Result<Vec<Memory>> {
        let source = self.get_memory_detail(agent_id.clone(), slug.clone()).await?;
        let source = match source {
            Some(s) => s,
            None => return Ok(vec![]),
        };

        let mut related = vec![];
        for rel_slug in &source.relations {
            if let Some(memory) = self
                .get_memory_detail(agent_id.clone(), rel_slug.clone())
                .await?
            {
                related.push(memory);
            }
        }

        let all_memories = self.get_all_agent_memory(agent_id.clone()).await?;
        for mem in all_memories {
            if mem.relations.contains(&slug) && !related.iter().any(|m| m.slug == mem.slug) {
                related.push(mem);
            }
        }

        Ok(related)
    }

    async fn get_memory_graph(&self, agent_id: String) -> Result<MemoryGraph> {
        let memories = self.get_all_agent_memory(agent_id.clone()).await?;

        let mut nodes: Vec<MemoryGraphNode> = memories
            .iter()
            .map(|m| MemoryGraphNode {
                slug: m.slug.trim_end_matches(".md").to_string(),
                title: m.title.clone(),
                tags: m.tags.clone(),
                visibility: m.visibility.clone(),
                agent_id: m.agent_id.clone(),
            })
            .collect();

        let slugs: std::collections::HashSet<String> = memories
            .iter()
            .map(|m| m.slug.trim_end_matches(".md").to_string())
            .collect();

        let mut edges = Vec::new();
        for memory in &memories {
            let source_slug = memory.slug.trim_end_matches(".md").to_string();
            for rel_slug in &memory.relations {
                let target_slug = rel_slug.trim_end_matches(".md").to_string();
                let broken = !slugs.contains(&target_slug);
                edges.push(MemoryGraphEdge {
                    source: source_slug.clone(),
                    target: target_slug,
                    broken,
                });
            }
        }

        nodes.sort_by(|a, b| a.slug.cmp(&b.slug));
        edges.sort_by(|a, b| a.source.cmp(&b.source).then(a.target.cmp(&b.target)));

        Ok(MemoryGraph { nodes, edges })
    }

    async fn delete_memory(
        &self,
        agent_id: String,
        slug: String,
        indexer: &VizierIndexer,
    ) -> Result<()> {
        let slug = if slug.ends_with(".md") {
            slug
        } else {
            format!("{}.md", slug)
        };

        for agent_dir in [&agent_id, GLOBAL_AGENT_ID] {
            let path = format!(
                "{}/agents/{}/{}/{}",
                self.workspace,
                agent_dir,
                MEMORY_PATH,
                slug
            );
            let pb = PathBuf::from(&path);
            if pb.exists() {
                let _ = indexer.delete_index("memory".into(), path).await;
                let _ = std::fs::remove_file(&pb);
            }
        }

        Ok(())
    }
}
