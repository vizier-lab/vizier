pub mod agent;
mod commands;
mod global_config;
mod history;
mod metrics;
pub mod provider;
mod request;
mod response;
mod session;
mod storage;
mod task;

pub use agent::{AgentConfig, AgentToolsConfig, BraveSearchToolSettings, MemoryConfig, ToolConfig};
pub use commands::{AgentCommand, AgentCommandResult, AgentSummary, CommandRequest, CommandResponse, GlobalCommand, GlobalCommandResult};
pub use global_config::{GlobalConfigEntry, GlobalConfigValue};
pub use history::{SessionHistory, SessionHistoryContent};
pub use metrics::{
    AgentUsageStats, ChannelTypeUsage, ChannelTypeUsageDetail, ChannelUsage, DailyChannelTypeUsage,
    DailyUsage, UsageSummary,
};
pub use provider::{ProviderEntry, ProviderEntryConfig};
pub use request::{VizierAttachment, VizierAttachmentContent, VizierRequest, VizierRequestContent};
pub use response::{VizierResponse, VizierResponseContent, VizierResponseStats};
pub use session::{AgentId, TopicId, VizierChannelId, VizierSession, VizierSessionDetail};
pub use storage::{DocumentIndex, Memory, SharedDocument, SharedDocumentSummary, Skill};
pub use task::{Task, TaskSchedule};

use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
struct User {
    pub username: String,
    pub password_hash: String,
}
