use anyhow::Result;
use chrono::Utc;
use regex::Regex;

use crate::{
    embedding::VizierEmbeddingModel,
    error::VizierError,
    schema::{
        Memory, MemoryGraph, MemoryGraphEdge, MemoryGraphNode, MemoryQueryParams, MemoryVisibility,
        PaginatedMemory,
    },
    storage::{
        memory::MemoryStorage,
        surreal::{DistanceFunction, SurrealStorage},
    },
};

use slugify::slugify;

const GLOBAL_AGENT_ID: &str = "_global";

fn parse_wikilinks(content: &str) -> Vec<String> {
    let re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
    re.captures_iter(content)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

#[async_trait::async_trait]
impl MemoryStorage for SurrealStorage {
    async fn write_memory(
        &self,
        agent_id: String,
        slug: Option<String>,
        title: String,
        content: String,
        visibility: MemoryVisibility,
        shared_to: Vec<String>,
        tags: Vec<String>,
    ) -> Result<Memory> {
        let embedder = self
            .embedder
            .clone()
            .ok_or(VizierError("embedder is not set".into()))?;

        let slug = slug.unwrap_or_else(|| slugify!(&title));
        let store_agent_id = match visibility {
            MemoryVisibility::Global => GLOBAL_AGENT_ID.to_string(),
            _ => agent_id.clone(),
        };

        let new_key = format!("{}/{}", store_agent_id, slug);
        for old_agent_dir in [&agent_id, GLOBAL_AGENT_ID] {
            let old_key = format!("{}/{}", old_agent_dir, slug);
            if old_key != new_key {
                let _ = self
                    .conn
                    .delete::<Option<Memory>>(("memory", old_key))
                    .await;
            }
        }

        let relations = parse_wikilinks(&content);

        let mut memory = Memory {
            slug: slug.clone(),
            agent_id: store_agent_id.clone(),
            title,
            content: content.clone(),
            timestamp: Utc::now(),
            embedding: vec![],
            visibility,
            shared_to,
            tags,
            keywords: vec![],
            relations,
        };

        let embedding = embedder.embed_text(&content).await?;
        memory.embedding = embedding;

        let _: Option<Memory> = self
            .conn
            .upsert(("memory", format!("{}/{}", store_agent_id, slug)))
            .content(memory.clone())
            .await?;

        Ok(memory)
    }

    async fn query_memory(
        &self,
        agent_id: String,
        query: String,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<Memory>> {
        let embedder = self
            .embedder
            .clone()
            .ok_or(VizierError("embedder is not set".into()))?;

        let query = embedder.embed_text(&query).await?;

        let distance_function = DistanceFunction::Cosine;

        let mut response = self
            .conn
            .query(format!(
                r#"SELECT *
                    FROM type::table($table)
                    WHERE {distance_function}($query, embedding) >= $threshold
                        AND (
                            visibility = 'private' AND agent_id = $agent_id
                            OR visibility = 'global'
                            OR (visibility = 'shared' AND array::contains(shared_to, $agent_id))
                        )
                    ORDER BY distance ASC
                    LIMIT $limit"#
            ))
            .bind(("table", "memory"))
            .bind(("agent_id", agent_id))
            .bind(("query", query))
            .bind(("limit", limit))
            .bind(("threshold", threshold))
            .await?;

        let res: Vec<Memory> = response.take(0)?;

        Ok(res)
    }

    async fn get_all_agent_memory(&self, agent_id: String) -> Result<Vec<Memory>> {
        let mut response = self
            .conn
            .query(
                r#"SELECT * FROM type::table(memory)
                    WHERE visibility = 'private' AND agent_id = $agent_id
                    OR visibility = 'global'
                    OR (visibility = 'shared' AND array::contains(shared_to, $agent_id))"#,
            )
            .bind(("agent_id", agent_id))
            .await?;

        let data: Vec<Memory> = response.take(0)?;

        Ok(data)
    }

    async fn get_filtered_memories(
        &self,
        params: MemoryQueryParams,
    ) -> Result<PaginatedMemory> {
        let agent_id = &params.agent_id;

        let mut where_clauses = vec![
            "(visibility = 'private' AND agent_id = $agent_id)".to_string(),
            "visibility = 'global'".to_string(),
            "(visibility = 'shared' AND array::contains(shared_to, $agent_id))".to_string(),
        ];

        if let Some(ref tags) = params.tags {
            if !tags.is_empty() {
                let tag_conditions: Vec<String> = tags
                    .iter()
                    .map(|t| format!("array::contains(tags, '{}')", t.replace('\'', "''")))
                    .collect();
                where_clauses.push(format!("({})", tag_conditions.join(" OR ")));
            }
        }

        if let Some(visibility) = &params.visibility {
            where_clauses = where_clauses
                .into_iter()
                .filter(|c| c.contains(&format!("visibility = '{}'", visibility)))
                .collect();
        }

        let where_sql = where_clauses.join(" OR ");

        let sort_field = params.sort_by.as_deref().unwrap_or("timestamp");
        let sort_dir = if params.sort_order.as_deref() == Some("asc") {
            "ASC"
        } else {
            "DESC"
        };

        let count_query = format!(
            "SELECT count() FROM type::table(memory) WHERE {} GROUP ALL",
            where_sql
        );

        let mut count_response = self
            .conn
            .query(&count_query)
            .bind(("agent_id", agent_id.clone()))
            .await?;

        let count_result: Option<serde_json::Value> = count_response.take(0)?;
        let total = count_result
            .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
            .unwrap_or(0) as usize;

        let data_query = format!(
            "SELECT * FROM type::table(memory)
                WHERE {}
                ORDER BY {} {}
                START {} LIMIT {}",
            where_sql, sort_field, sort_dir, params.offset, params.limit
        );

        let mut data_response = self
            .conn
            .query(&data_query)
            .bind(("agent_id", agent_id.clone()))
            .await?;

        let memories: Vec<Memory> = data_response.take(0)?;

        Ok(PaginatedMemory {
            memories,
            total,
            offset: params.offset,
            limit: params.limit,
        })
    }

    async fn get_memory_detail(&self, agent_id: String, slug: String) -> Result<Option<Memory>> {
        let mut response = self
            .conn
            .query(
                r#"SELECT * FROM type::table(memory)
                    WHERE slug = $slug
                        AND (
                            visibility = 'private' AND agent_id = $agent_id
                            OR visibility = 'global'
                            OR (visibility = 'shared' AND array::contains(shared_to, $agent_id))
                        )"#,
            )
            .bind(("slug", slug))
            .bind(("agent_id", agent_id))
            .await?;

        let data: Option<Memory> = response.take(0)?;

        Ok(data)
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

        let mut response = self
            .conn
            .query(
                r#"SELECT * FROM type::table(memory)
                    WHERE array::contains(relations, $slug)
                        AND (
                            visibility = 'private' AND agent_id = $agent_id
                            OR visibility = 'global'
                            OR (visibility = 'shared' AND array::contains(shared_to, $agent_id))
                        )"#,
            )
            .bind(("slug", slug))
            .bind(("agent_id", agent_id))
            .await?;

        let incoming: Vec<Memory> = response.take(0)?;
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
                slug: m.slug.clone(),
                title: m.title.clone(),
                tags: m.tags.clone(),
                visibility: m.visibility.clone(),
                agent_id: m.agent_id.clone(),
            })
            .collect();

        let slugs: std::collections::HashSet<String> = memories.iter().map(|m| m.slug.clone()).collect();

        let mut edges = Vec::new();
        for memory in &memories {
            for rel_slug in &memory.relations {
                let broken = !slugs.contains(rel_slug);
                edges.push(MemoryGraphEdge {
                    source: memory.slug.clone(),
                    target: rel_slug.clone(),
                    broken,
                });
            }
        }

        nodes.sort_by(|a, b| a.slug.cmp(&b.slug));
        edges.sort_by(|a, b| a.source.cmp(&b.source).then(a.target.cmp(&b.target)));

        Ok(MemoryGraph { nodes, edges })
    }

    async fn delete_memory(&self, agent_id: String, slug: String) -> Result<()> {
        let detail = self.get_memory_detail(agent_id.clone(), slug.clone()).await?;
        let actual_agent_id = match detail {
            Some(m) => m.agent_id,
            None => agent_id,
        };
        let _ = self
            .conn
            .delete::<Option<Memory>>(("memory", format!("{}/{}", actual_agent_id, slug)))
            .await?;

        Ok(())
    }
}
