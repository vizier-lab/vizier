use anyhow::Result;
use chrono::Utc;
use regex::Regex;

use crate::{
    indexer::VizierIndexer,
    schema::{
        Memory, MemoryGraph, MemoryGraphEdge, MemoryGraphNode, MemoryQueryParams,
        MemoryVisibility, PaginatedMemory, VizierAttachment,
    },
    storage::{memory::MemoryStorage, sqlite::SqliteStorage},
};

use slugify::slugify;

const GLOBAL_AGENT_ID: &str = "_global";

fn parse_wikilinks(content: &str) -> Vec<String> {
    let re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
    re.captures_iter(content)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn can_access_memory(agent_id: &str, memory: &Memory) -> bool {
    match memory.visibility {
        MemoryVisibility::Private => memory.agent_id == agent_id,
        MemoryVisibility::Global => true,
        MemoryVisibility::Shared => memory.shared_to.contains(&agent_id.to_string()),
    }
}

#[async_trait::async_trait]
impl MemoryStorage for SqliteStorage {
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

        let new_key = format!("{}/{}", store_agent_id, slug);

        // Clean up old records if key changed
        for old_agent_dir in [&agent_id, GLOBAL_AGENT_ID] {
            let old_key = format!("{}/{}", old_agent_dir, slug);
            if old_key != new_key {
                {
                    let conn = self.conn.lock();
                    let _ = conn.execute(
                        "DELETE FROM memory WHERE id = ?1",
                        rusqlite::params![old_key],
                    );
                }
                let _ = indexer
                    .delete_index("memory".into(), format!("memory/{}/{}", old_agent_dir, slug))
                    .await;
            }
        }

        let relations = parse_wikilinks(&content);

        let memory = Memory {
            slug: slug.clone(),
            agent_id: store_agent_id.clone(),
            title,
            content: content.clone(),
            timestamp: Utc::now(),
            visibility,
            shared_to,
            tags,
            keywords: vec![],
            relations,
            attachments,
        };

        let data = serde_json::to_string(&memory)?;
        {
            let conn = self.conn.lock();
            conn.execute(
                "INSERT OR REPLACE INTO memory (id, agent_id, slug, visibility, data) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    new_key,
                    store_agent_id,
                    slug,
                    serde_json::to_string(&memory.visibility)?.trim_matches('"'),
                    data
                ],
            )?;
        }

        indexer
            .add_document_index(
                "memory".into(),
                format!("memory/{}", new_key),
                content,
            )
            .await?;

        Ok(memory)
    }

    async fn query_memory(
        &self,
        agent_id: String,
        query: String,
        limit: usize,
        threshold: f64,
        indexer: &VizierIndexer,
    ) -> Result<Vec<Memory>> {
        let _ = threshold;

        let documents = indexer
            .search_document_index("memory".into(), query, limit * 3, 0.0)
            .await?;

        let mut res = vec![];
        for doc in documents {
            let slug_part = doc.path.rsplit('/').next().unwrap_or(&doc.path);
            if let Some(memory) = self
                .get_memory_detail(agent_id.clone(), slug_part.to_string())
                .await?
            {
                res.push(memory);
            }

            if res.len() >= limit {
                break;
            }
        }

        Ok(res)
    }

    async fn get_all_agent_memory(&self, agent_id: String) -> Result<Vec<Memory>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM memory")?;
        let memories: Vec<Memory> = stmt
            .query_map([], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<Memory>(&data).ok())
            .filter(|m| can_access_memory(&agent_id, m))
            .collect();
        Ok(memories)
    }

    async fn get_filtered_memories(
        &self,
        params: MemoryQueryParams,
    ) -> Result<PaginatedMemory> {
        let all_memories = self.get_all_agent_memory(params.agent_id.clone()).await?;

        let mut filtered: Vec<Memory> = all_memories
            .into_iter()
            .filter(|m| {
                if let Some(ref visibility) = params.visibility
                    && &m.visibility != visibility
                {
                    return false;
                }

                if let Some(ref tags) = params.tags
                    && !tags.is_empty()
                    && !tags.iter().any(|t| m.tags.contains(t))
                {
                    return false;
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
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM memory WHERE slug = ?1")?;
        let memories: Vec<Memory> = stmt
            .query_map(rusqlite::params![slug], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<Memory>(&data).ok())
            .filter(|m| can_access_memory(&agent_id, m))
            .collect();

        Ok(memories.into_iter().next())
    }

    async fn get_related_memories(
        &self,
        agent_id: String,
        slug: String,
    ) -> Result<Vec<Memory>> {
        let source = self
            .get_memory_detail(agent_id.clone(), slug.clone())
            .await?;
        let source = match source {
            Some(s) => s,
            None => return Ok(vec![]),
        };

        if source.relations.is_empty() {
            return Ok(vec![]);
        }

        let mut related = vec![];
        for rel_slug in &source.relations {
            if let Some(memory) = self
                .get_memory_detail(agent_id.clone(), rel_slug.clone())
                .await?
            {
                related.push(memory);
            }
        }

        // Find memories that reference this slug (incoming relations)
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM memory")?;
        let incoming: Vec<Memory> = stmt
            .query_map([], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?
            .filter_map(|r| r.ok())
            .filter_map(|data| serde_json::from_str::<Memory>(&data).ok())
            .filter(|m| m.relations.contains(&slug) && can_access_memory(&agent_id, m))
            .collect();

        for mem in incoming {
            if !related.iter().any(|m| m.slug == mem.slug) {
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
        // Find the actual memory to get the correct agent_id
        let detail = self
            .get_memory_detail(agent_id.clone(), slug.clone())
            .await?;
        let actual_agent_id = match detail {
            Some(m) => m.agent_id,
            None => agent_id,
        };

        let key = format!("{}/{}", actual_agent_id, slug);
        {
            let conn = self.conn.lock();
            let _ = conn.execute(
                "DELETE FROM memory WHERE id = ?1",
                rusqlite::params![key],
            );
        }
        let _ = indexer
            .delete_index("memory".into(), format!("memory/{}", key))
            .await;

        Ok(())
    }
}
