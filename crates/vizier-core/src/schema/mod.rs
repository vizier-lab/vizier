pub mod agent;
pub mod artifact;
mod commands;
pub mod detached;
pub mod dream_journal;
mod file;
mod global_config;
mod history;
pub mod integration;
pub mod metrics;
pub mod notification;
pub mod project;
pub mod workspace;
pub mod provider;
mod request;
mod response;
pub mod revision;
pub mod secret;
mod session;
pub mod session_file;
/// Subagents as temporary workers (spec 059).
pub mod subagent;
/// The Bedrock SSO session: the non-secret view and the encrypted payload (spec 044).
pub mod sso;
mod storage;
mod task;
pub mod usage;

pub use agent::{
    AgentConfig, AgentToolsConfig, EmbeddingConfig, EmbeddingProvider, ImageGenProvider,
    ImageGenToolSettings, IndexerConfig, IndexerKind, ReadImageToolSettings, SttToolSettings,
    ToolConfig, TtsToolSettings,
};
pub use artifact::{
    Artifact, ArtifactCursor, ArtifactDeleteResult, ArtifactEvent, ArtifactEventAction,
    ArtifactFilters, ArtifactId, ArtifactKind, ArtifactOrigin, ArtifactPage, ArtifactReference,
    ArtifactStats, validate_artifact_name,
};
pub use detached::{
    Canceller, DetachedScript, DetachedScriptDetail, DetachedScriptId, DetachedScriptSnapshot,
    DetachedScriptState, new_detached_id,
};
pub use commands::{
    AgentCommand, AgentCommandResult, AgentHealthStatus, AgentModelInfo, AgentRunState,
    AgentSummary, ChannelHealth, ChannelPlatform, ChannelState, CommandRequest, CommandResponse,
    MemoryOpRequest, MemoryOpResponse,
};
pub use subagent::{
    Subagent, SubagentBrief, SubagentId, SubagentReplyText, SubagentSnapshot, SubagentState, SubagentStopReason,
    SubagentSummary,
};
pub use dream_journal::{DreamJournalEntry, DreamJournalEntryFrontMatter};
pub use file::FileRecord;
pub use global_config::{GlobalConfigEntry, GlobalConfigValue};
pub use history::{
    ClientCallOutcome, RecallOrigin, RecalledPassage, SessionHistory, SessionHistoryContent,
    history_entries_to_messages, message_for_model, messages_for_model,
    messages_to_history_entries, open_tool_calls, seal_tool_calls, split_at_request,
};
pub use integration::{
    EventAction, EventDraft, Integration, IntegrationChanged, IntegrationEvent, IntegrationFamily,
    IntegrationGrant, IntegrationStatus,
};
pub use notification::{
    ChatUnread, InsertedNotification, NotificationCategory, WORKSPACE_SCOPE_PREFIX, workspace_scope, NotificationCounts, NotificationDraft,
    NotificationFilter, NotificationItem, NotificationPage, NotificationTone,
};
pub use project::{
    DEFAULT_COMMAND_TIMEOUT, INSTRUCTIONS_CAP_BYTES, MAX_COMMAND_TIMEOUT, NEW_SESSION_TITLE,
    LspCapacity, LspServerStatus, LspStatusSnapshot, OUTPUT_CAP_BYTES, PROJECT_ID_LEN, Project,
    ProjectMode, ProjectScope, ProjectSessionRow, ServerOverride, Worktree, WorktreeRef,
};
pub use workspace::{
    CatchUp, EntryFile, EntryKind, EntryMention, MemberKind, MentionState, NewEntry, Workspace,
    WorkspaceEntry, WorkspaceMember, WorkspaceRole, WorkspaceTopic,
};
pub use provider::{
    CustomModel, CustomProtocol, CustomProvider, LimitsEdited, ProfileEntry, ProviderCredentials,
    ProviderEntry,
};
pub use request::{
    AttachmentResolver, ClientEvent, ModelOverride, NoopAttachmentResolver, PlatformMessageId,
    ReactionAction, ReactionEntry, SideChat, SideChatTurn, VizierAttachment,
    VizierAttachmentContent, VizierRequest, VizierRequestContent, attachments_note,
};
pub use response::{ErrorKind, VizierResponse, VizierResponseContent, VizierResponseStats};
pub use revision::{
    CoreRevision, CoreRevisionSummary, DiffHunk, DiffLine, MemoryRevision, MemoryRevisionSummary,
    PaginatedCoreRevisions, PaginatedMemoryRevisions, RevisionActor, RevisionDiff, RevisionOrigin,
    RevisionTrigger, RollbackResponse,
};
pub use secret::{AgentSecretEvent, SecretAction};
pub use sso::{SsoSession, SsoSessionSecret, SsoSessionState};
pub use session::{
    AgentId, DreamStage, DreamStatus, TopicId, VizierChannelId, VizierSession, VizierSessionDetail,
};
pub use session_file::SessionFileRecord;
pub use storage::{
    BundleSummary, CORE_BUNDLE, DEFAULT_BUNDLE, DocumentIndex, ImportReport, Memory,
    MemoryAttachment, MemoryAttachmentView, MemoryFrontMatter, MemoryGraph, MemoryGraphEdge,
    MemoryGraphNode, MemoryPassageResult, MemoryQueryParams, PaginatedMemory, SOUL_PATH, Skill,
    SkillFrontMatter, SkillVersion, default_bundle, is_core_bundle, is_identity_address,
};
pub use task::{Requester, Task, TaskFrontMatter, TaskRun, TaskRunState, TaskSchedule};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct User {
    pub username: String,
    pub password_hash: String,
}
