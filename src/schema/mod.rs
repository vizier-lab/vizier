pub mod agent;
mod commands;
pub mod context_file;
pub mod dream_journal;
mod file;
mod global_config;
mod history;
mod metrics;
pub mod provider;
mod request;
mod response;
mod session;
mod storage;
mod task;

pub use agent::{AgentConfig, AgentToolsConfig, BraveSearchToolSettings, MemoryConfig, ToolConfig, TtsToolSettings};
pub use context_file::ContextFileRecord;
pub use dream_journal::DreamJournalEntry;
pub use file::FileRecord;
pub use commands::{AgentCommand, AgentCommandResult, AgentHealthStatus, AgentSummary, ChannelCommand, CommandRequest, CommandResponse, FileCommand};
pub use global_config::{GlobalConfigEntry, GlobalConfigValue};
pub use history::{SessionHistory, SessionHistoryContent};
pub use metrics::{
    AgentUsageStats, ChannelTypeUsage, ChannelTypeUsageDetail, ChannelUsage, DailyChannelTypeUsage,
    DailyUsage, UsageSummary,
};
pub use provider::{ProviderEntry, ProviderEntryConfig, Quantization};
pub use request::{PlatformMessageId, ReactionAction, ReactionEntry, ReactionEvent, VizierAttachment, VizierAttachmentContent, VizierRequest, VizierRequestContent};
pub use response::{VizierResponse, VizierResponseContent, VizierResponseStats};
pub use session::{AgentId, DreamStage, DreamStatus, TopicId, VizierChannelId, VizierSession, VizierSessionDetail};
pub use storage::{DocumentIndex, Memory, MemoryGraph, MemoryGraphEdge, MemoryGraphNode, MemoryQueryParams, MemoryVisibility, PaginatedMemory, Skill, SkillActivation};
pub use task::{Task, TaskSchedule};

use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
struct User {
    pub username: String,
    pub password_hash: String,
}
