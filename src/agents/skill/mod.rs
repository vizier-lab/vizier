use anyhow::Result;

use crate::{
    dependencies::VizierDependencies,
    indexer::VizierIndexer,
    schema::{AgentId, Skill},
    skill::SkillManager,
};

const GLOBAL_SCOPE: &str = "global";
const INDEX_CONTEXT: &str = "skill";

#[derive(Clone)]
pub struct VizierSkills {
    agent_id: String,
    global_manager: SkillManager,
    agent_manager: SkillManager,
    indexer: Option<VizierIndexer>,
}

fn agent_scope(agent_id: &str) -> String {
    format!("agent/{agent_id}")
}

fn index_key(scope: &str, slug: &str) -> String {
    format!("{scope}/{slug}")
}

fn embed_text(skill: &Skill) -> String {
    format!(
        "{}\n{}\nkeywords: {}",
        skill.name,
        skill.description,
        skill.keywords.join(", ")
    )
}

impl VizierSkills {
    pub async fn new(
        agent_id: AgentId,
        deps: VizierDependencies,
        indexer: Option<VizierIndexer>,
    ) -> Result<Self> {
        let workspace = deps.config.workspace.clone();
        Ok(Self {
            agent_id: agent_id.clone(),
            global_manager: SkillManager::new(&workspace),
            agent_manager: SkillManager::for_agent(&workspace, &agent_id),
            indexer,
        })
    }

    fn all_skills(&self) -> crate::Result<Vec<Skill>> {
        let mut skills = self.global_manager.list_skills()?;
        let agent_skills = self.agent_manager.list_skills()?;

        // Agent skills override global skills with same name
        for agent_skill in agent_skills {
            if !skills.iter().any(|s| s.name == agent_skill.name) {
                skills.push(agent_skill);
            }
        }

        Ok(skills)
    }

    pub async fn get_skill_content(&self, slug: String) -> Result<Option<String>> {
        // Agent skill takes priority
        if let Some(skill) = self.agent_manager.get_skill(&slug)? {
            return Ok(Some(skill.content));
        }
        if let Some(skill) = self.global_manager.get_skill(&slug)? {
            return Ok(Some(skill.content));
        }
        Ok(None)
    }

    pub fn get_skill_manager(&self) -> &SkillManager {
        &self.global_manager
    }

    pub fn get_agent_skill_manager(&self) -> &SkillManager {
        &self.agent_manager
    }

    /// Best-effort re-embed of every skill (global + agent-private). Called once at agent
    /// boot; individual create/update/delete tool calls keep the index fresh in between.
    pub async fn reindex_all(&self) -> Result<()> {
        let Some(indexer) = &self.indexer else {
            return Ok(());
        };

        for skill in self.global_manager.list_skills()? {
            let _ = indexer
                .add_document_index(
                    INDEX_CONTEXT.into(),
                    index_key(GLOBAL_SCOPE, &skill.name),
                    embed_text(&skill),
                )
                .await;
        }

        let scope = agent_scope(&self.agent_id);
        for skill in self.agent_manager.list_skills()? {
            let _ = indexer
                .add_document_index(
                    INDEX_CONTEXT.into(),
                    index_key(&scope, &skill.name),
                    embed_text(&skill),
                )
                .await;
        }

        Ok(())
    }

    pub async fn recommend_skills(
        &self,
        query: &str,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<Skill>> {
        let Some(indexer) = &self.indexer else {
            return Ok(vec![]);
        };

        let docs = indexer
            .search_document_index(INDEX_CONTEXT.into(), query.into(), limit * 5, threshold)
            .await?;

        let agent_scope_prefix = format!("{}/", agent_scope(&self.agent_id));
        let global_scope_prefix = format!("{GLOBAL_SCOPE}/");

        let mut seen = std::collections::HashSet::new();
        let mut recommended = Vec::new();

        for doc in docs {
            let (manager, slug) = if let Some(slug) = doc.path.strip_prefix(&agent_scope_prefix) {
                (&self.agent_manager, slug)
            } else if let Some(slug) = doc.path.strip_prefix(&global_scope_prefix) {
                (&self.global_manager, slug)
            } else {
                continue;
            };

            if !seen.insert(slug.to_string()) {
                continue;
            }

            if let Ok(Some(skill)) = manager.get_skill(slug) {
                recommended.push(skill);
            }

            if recommended.len() >= limit {
                break;
            }
        }

        Ok(recommended)
    }
}

/// Indexing helpers used by the skill-authoring tools (create/update/delete) so the
/// vector index stays in sync without waiting for the next agent-boot backfill.
pub async fn index_skill(indexer: &VizierIndexer, scope: &str, skill: &Skill) -> Result<()> {
    indexer
        .add_document_index(
            INDEX_CONTEXT.into(),
            index_key(scope, &skill.name),
            embed_text(skill),
        )
        .await?;
    Ok(())
}

pub async fn deindex_skill(indexer: &VizierIndexer, scope: &str, slug: &str) -> Result<()> {
    indexer
        .delete_index(INDEX_CONTEXT.into(), index_key(scope, slug))
        .await?;
    Ok(())
}

pub fn global_scope() -> &'static str {
    GLOBAL_SCOPE
}

pub fn scope_for_agent(agent_id: &str) -> String {
    agent_scope(agent_id)
}
