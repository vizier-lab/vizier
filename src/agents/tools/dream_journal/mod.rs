use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::VizierTool,
    error::VizierError,
    schema::{
        AgentId,
        DreamStage,
        dream_journal::DreamJournalEntry,
    },
    storage::{VizierStorage, dream_journal::DreamJournalStorage},
};

pub struct WriteDreamJournal {
    pub agent_id: AgentId,
    pub storage: Arc<VizierStorage>,
    pub dream_cycle_id: String,
    pub source_sessions: Vec<String>,
    pub session_context: Option<String>,
    pub provider_used: Option<String>,
    pub model_used: Option<String>,
    pub start_time: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct WriteDreamJournalArgs {
    #[schemars(description = "The dream stage: 'extraction' or 'consolidation'")]
    pub stage: String,

    #[schemars(description = "The LLM-generated dream report content")]
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct WriteDreamJournalOutput {
    #[schemars(description = "The ID of the saved dream journal entry")]
    pub entry_id: String,
}

#[async_trait::async_trait]
impl VizierTool for WriteDreamJournal {
    type Input = WriteDreamJournalArgs;
    type Output = WriteDreamJournalOutput;

    fn name() -> String {
        "write_dream_journal".into()
    }

    fn description(&self) -> String {
        "Write a dream journal entry. Use this to save your extraction or consolidation report during a dream cycle.".to_string()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let stage = match args.stage.as_str() {
            "extraction" => DreamStage::Extraction,
            "consolidation" => DreamStage::Consolidation,
            other => {
                return Err(VizierError(format!(
                    "Invalid stage '{}'. Must be 'extraction' or 'consolidation'.",
                    other
                )));
            }
        };

        let now = Utc::now();
        let duration_ms = (now - self.start_time).num_milliseconds().max(0) as u64;

        let entry = DreamJournalEntry {
            id: uuid::Uuid::new_v4().to_string(),
            dream_cycle_id: self.dream_cycle_id.clone(),
            agent_id: self.agent_id.clone(),
            timestamp: now,
            stage,
            source_sessions: self.source_sessions.clone(),
            session_context: self.session_context.clone(),
            content: args.content,
            duration_ms: Some(duration_ms),
            provider_used: self.provider_used.clone(),
            model_used: self.model_used.clone(),
        };

        let entry_id = entry.id.clone();
        self.storage.save_dream_entry(entry).await.map_err(|e| {
            VizierError(format!("Failed to save dream journal entry: {}", e))
        })?;

        Ok(WriteDreamJournalOutput { entry_id })
    }
}

pub struct ReadDreamJournal {
    pub agent_id: AgentId,
    pub storage: Arc<VizierStorage>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ReadDreamJournalArgs {
    #[schemars(description = "Maximum number of entries to return (default 10)")]
    pub limit: Option<usize>,

    #[schemars(description = "Filter by stage: 'extraction' or 'consolidation'")]
    pub stage: Option<String>,

    #[schemars(description = "Filter by dream cycle ID")]
    pub cycle_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DreamJournalSummary {
    pub id: String,
    pub dream_cycle_id: String,
    pub timestamp: DateTime<Utc>,
    pub stage: String,
    pub session_context: Option<String>,
    pub duration_ms: Option<u64>,
    pub content_summary: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ReadDreamJournalOutput {
    pub entries: Vec<DreamJournalSummary>,
}

#[async_trait::async_trait]
impl VizierTool for ReadDreamJournal {
    type Input = ReadDreamJournalArgs;
    type Output = ReadDreamJournalOutput;

    fn name() -> String {
        "read_dream_journal".into()
    }

    fn description(&self) -> String {
        "Read dream journal entries. Returns recent dream entries with summaries. Use this to reference previous dream extractions or consolidations.".to_string()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let stage_filter = args.stage.and_then(|s| match s.as_str() {
            "extraction" => Some(DreamStage::Extraction),
            "consolidation" => Some(DreamStage::Consolidation),
            _ => None,
        });

        let entries = if let Some(cycle_id) = args.cycle_id {
            self.storage
                .list_dream_entries_by_cycle(
                    self.agent_id.clone(),
                    &cycle_id,
                    stage_filter,
                )
                .await
        } else {
            self.storage
                .list_dream_entries(
                    self.agent_id.clone(),
                    Some(args.limit.unwrap_or(10)),
                    None,
                )
                .await
        }
        .map_err(|e| VizierError(format!("Failed to read dream journal: {}", e)))?;

        let summaries = entries
            .into_iter()
            .map(|e| {
                let content_summary = if e.content.len() > 200 {
                    format!("{}...", &e.content[..200])
                } else {
                    e.content.clone()
                };
                DreamJournalSummary {
                    id: e.id,
                    dream_cycle_id: e.dream_cycle_id,
                    timestamp: e.timestamp,
                    stage: match e.stage {
                        DreamStage::Extraction => "extraction".to_string(),
                        DreamStage::Consolidation => "consolidation".to_string(),
                    },
                    session_context: e.session_context,
                    duration_ms: e.duration_ms,
                    content_summary,
                }
            })
            .collect();

        Ok(ReadDreamJournalOutput { entries: summaries })
    }
}
