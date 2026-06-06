use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    schema::{
        AgentId,
        DreamStage,
        dream_journal::DreamJournalEntry,
    },
    storage::{
        dream_journal::DreamJournalStorage,
        fs::FileSystemStorage,
    },
    utils,
};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
struct DreamJournalFrontMatter {
    pub id: String,
    pub dream_cycle_id: String,
    pub agent_id: AgentId,
    pub timestamp: DateTime<Utc>,
    pub stage: DreamStage,
    pub source_sessions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_context: Option<String>,
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_used: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_used: Option<String>,
}

impl From<&DreamJournalEntry> for DreamJournalFrontMatter {
    fn from(entry: &DreamJournalEntry) -> Self {
        Self {
            id: entry.id.clone(),
            dream_cycle_id: entry.dream_cycle_id.clone(),
            agent_id: entry.agent_id.clone(),
            timestamp: entry.timestamp,
            stage: entry.stage.clone(),
            source_sessions: entry.source_sessions.clone(),
            session_context: entry.session_context.clone(),
            duration_ms: entry.duration_ms,
            provider_used: entry.provider_used.clone(),
            model_used: entry.model_used.clone(),
        }
    }
}

fn entry_to_path(workspace: &str, agent_id: &str, entry: &DreamJournalEntry) -> PathBuf {
    PathBuf::from(format!(
        "{}/agents/{}/dreams/{}_{}.md",
        workspace,
        agent_id,
        entry.timestamp.to_rfc3339(),
        entry.id
    ))
}

fn dreams_dir(workspace: &str, agent_id: &str) -> String {
    format!("{}/agents/{}/dreams/*.md", workspace, agent_id)
}

fn read_entry_from_path(path: PathBuf) -> Result<DreamJournalEntry> {
    let (fm, content) = utils::markdown::read_markdown::<DreamJournalFrontMatter>(path)?;
    Ok(DreamJournalEntry {
        id: fm.id,
        dream_cycle_id: fm.dream_cycle_id,
        agent_id: fm.agent_id,
        timestamp: fm.timestamp,
        stage: fm.stage,
        source_sessions: fm.source_sessions,
        session_context: fm.session_context,
        content,
        duration_ms: fm.duration_ms,
        provider_used: fm.provider_used,
        model_used: fm.model_used,
    })
}

#[async_trait::async_trait]
impl DreamJournalStorage for FileSystemStorage {
    async fn save_dream_entry(&self, entry: DreamJournalEntry) -> Result<()> {
        let frontmatter = DreamJournalFrontMatter::from(&entry);
        let path = entry_to_path(&self.workspace, &entry.agent_id, &entry);
        utils::markdown::write_markdown(&frontmatter, entry.content, path)?;
        Ok(())
    }

    async fn list_dream_entries(
        &self,
        agent_id: AgentId,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<DreamJournalEntry>> {
        let pattern = dreams_dir(&self.workspace, &agent_id);
        let mut entries: Vec<DreamJournalEntry> = vec![];

        for glob_entry in glob::glob(&pattern)? {
            let glob_entry = glob_entry?;
            if !glob_entry.is_file() {
                continue;
            }
            if let Ok(entry) = read_entry_from_path(glob_entry) {
                entries.push(entry);
            }
        }

        entries.sort_by_key(|b| std::cmp::Reverse(b.timestamp));

        let offset = offset.unwrap_or(0);
        if offset >= entries.len() {
            return Ok(vec![]);
        }
        entries = entries[offset..].to_vec();

        if let Some(limit) = limit {
            entries.truncate(limit);
        }

        Ok(entries)
    }

    async fn get_dream_entry(
        &self,
        agent_id: AgentId,
        entry_id: String,
    ) -> Result<Option<DreamJournalEntry>> {
        let pattern = dreams_dir(&self.workspace, &agent_id);

        for glob_entry in glob::glob(&pattern)? {
            let glob_entry = glob_entry?;
            if !glob_entry.is_file() {
                continue;
            }
            if let Ok(entry) = read_entry_from_path(glob_entry)
                && entry.id == entry_id
            {
                return Ok(Some(entry));
            }
        }

        Ok(None)
    }

    async fn get_latest_dream_entry(
        &self,
        agent_id: AgentId,
        stage: DreamStage,
    ) -> Result<Option<DreamJournalEntry>> {
        let pattern = dreams_dir(&self.workspace, &agent_id);
        let mut entries: Vec<DreamJournalEntry> = vec![];

        for glob_entry in glob::glob(&pattern)? {
            let glob_entry = glob_entry?;
            if !glob_entry.is_file() {
                continue;
            }
            if let Ok(entry) = read_entry_from_path(glob_entry)
                && entry.stage == stage
            {
                entries.push(entry);
            }
        }

        entries.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
        Ok(entries.into_iter().next())
    }

    async fn list_dream_entries_by_cycle(
        &self,
        agent_id: AgentId,
        cycle_id: &str,
        stage: Option<DreamStage>,
    ) -> Result<Vec<DreamJournalEntry>> {
        let pattern = dreams_dir(&self.workspace, &agent_id);
        let mut entries: Vec<DreamJournalEntry> = vec![];

        for glob_entry in glob::glob(&pattern)? {
            let glob_entry = glob_entry?;
            if !glob_entry.is_file() {
                continue;
            }
            if let Ok(entry) = read_entry_from_path(glob_entry)
                && entry.dream_cycle_id == cycle_id
            {
                if let Some(ref filter_stage) = stage {
                    if entry.stage == *filter_stage {
                        entries.push(entry);
                    }
                } else {
                    entries.push(entry);
                }
            }
        }

        entries.sort_by_key(|a| a.timestamp);
        Ok(entries)
    }
}
