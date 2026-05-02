mod commands;
mod history;
mod metrics;
mod request;
mod response;
mod session;
mod storage;
mod task;

pub use commands::{CommandRequest, CommandResponse};
pub use history::{SessionHistory, SessionHistoryContent};
pub use metrics::{
    AgentUsageStats, ChannelTypeUsage, ChannelTypeUsageDetail, ChannelUsage, DailyChannelTypeUsage,
    DailyUsage, UsageSummary,
};
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
