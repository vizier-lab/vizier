// API Response wrapper
export interface ApiResponse<T> {
  status: number
  message?: string
  data: T
}

// ============================================================================
// AUTH
// ============================================================================

export interface LoginResponse {
  token: string
}

export interface SetupStatus {
  needs_setup: boolean
}

export interface ApiKey {
  id: string
  name: string
  key?: string // Only returned on creation
  expires_at?: string
  created_at: string
  last_used_at?: string
}

// ============================================================================
// RBAC (Role-Based Access Control)
// ============================================================================

export interface Role {
  role_id: string
  name: string
  permissions: string[]
  is_system: boolean
  created_at: string
}

export interface User {
  user_id: string
  username: string
  role_id: string
  role_name?: string
  created_at: string
}

export interface CurrentUser {
  user_id: string
  username: string
  role_name: string
  permissions: string[]
}

export interface UserProfile {
  user_id: string
  discord_id: string | null
  discord_username: string | null
  telegram_id: string | null
  telegram_username: string | null
  alias: string[]
}

export interface UpdateUserProfileRequest {
  discord_id?: string | null
  discord_username?: string | null
  telegram_id?: string | null
  telegram_username?: string | null
  alias?: string[]
}

export interface CreateUserRequest {
  username: string
  password: string
  role_id?: string
}

export interface UpdateUserRequest {
  username?: string
  role_id?: string
  password?: string
}

export interface AvailablePermissions {
  permissions: string[]
}

// ============================================================================
// AGENT
// ============================================================================

export interface Agent {
  agent_id: string
  name: string
  description?: string
  avatar_url?: string
  owner_username?: string
  owner_id?: string
  shared_to?: string[]
}

export interface AgentToolConfig {
  enabled: boolean
}

export interface BraveSearchToolSettings {
  api_key?: string
  safesearch?: boolean
}

export type TtsProvider = 'piper' | 'openai' | 'openrouter' | 'elevenlabs'

export interface TtsToolSettings {
  provider?: TtsProvider
  model?: string
  voice?: string
  speed?: number
}

export interface AgentToolsConfig {
  timeout: string
  programmatic_sandbox: boolean
  shell: ShellConfigData | null
  brave_search: AgentToolConfig
  brave_search_settings?: BraveSearchToolSettings
  discord: AgentToolConfig
  telegram: AgentToolConfig
  fetch: AgentToolConfig
  http_client: AgentToolConfig
  mcp_servers: Record<string, McpServerConfig>
  tts: AgentToolConfig
  tts_settings?: TtsToolSettings
}

export interface AgentConfig {
  name: string
  system_prompt?: string
  description?: string
  provider: string
  model: string
  session_memory: { max_capacity: number }
  thinking_depth: number
  tools: AgentToolsConfig
  silent_read_initiative_chance: number
  show_thinking?: boolean
  show_tool_calls?: boolean
  max_tokens?: number
  include_documents?: string[]
  prompt_timeout: string
  heartbeat_interval: string
  dream_enabled: boolean
  dream_schedule: string | null
  dream_provider: string | null
  dream_model: string | null
}

export interface CreateAgentRequest {
  agent_id: string
  name: string
  description?: string
  provider: string
  model: string
  quantization?: string
  system_prompt?: string
  thinking_depth?: number
  max_tokens?: number
  session_memory_capacity?: number
  show_thinking?: boolean
  show_tool_calls?: boolean
  silent_read_initiative_chance?: number
  tools?: {
    shell?: ShellConfigData | null
    brave_search?: boolean
    brave_search_settings?: BraveSearchToolSettings
    discord?: boolean
    telegram?: boolean
    fetch?: boolean
    http_client?: boolean
    programmatic_sandbox?: boolean
    timeout?: string
    mcp_servers?: Record<string, McpServerConfig>
    tts?: boolean
    tts_settings?: TtsToolSettings
  }
  prompt_timeout?: string
  heartbeat_interval?: string
  dream_enabled?: boolean
  dream_schedule?: string
  dream_provider?: string | null
  dream_model?: string | null
  discord_token?: string
  telegram_token?: string
  avatar_url?: string
}

export interface AgentDetail {
  agent_id: string
  name: string
  description?: string
  provider: string
  model: string
  quantization?: string
  system_prompt?: string
  thinking_depth: number
  max_tokens?: number
  session_memory_capacity: number
  show_thinking?: boolean
  show_tool_calls?: boolean
  silent_read_initiative_chance?: number
  shell: ShellConfigData | null
  brave_search: boolean
  brave_search_settings?: BraveSearchToolSettings
  discord: boolean
  telegram: boolean
  fetch: boolean
  http_client: boolean
  programmatic_sandbox?: boolean
  prompt_timeout: string
  heartbeat_interval: string
  dream_enabled: boolean
  dream_schedule: string | null
  dream_provider: string | null
  dream_model: string | null
  discord_token?: string
  telegram_token?: string
  tools_timeout: string
  mcp_servers: Record<string, McpServerConfig>
  tts: boolean
  tts_settings?: TtsToolSettings
  avatar_url?: string
}

// ============================================================================
// PROVIDERS
// ============================================================================

export interface ProviderResponse {
  variant: string
  has_api_key: boolean
  base_url?: string
  enabled?: boolean
}

export interface UpsertProviderRequest {
  api_key?: string
  base_url?: string
  enabled?: boolean
}

// ============================================================================
// CHAT/TOPIC
// ============================================================================

export interface Topic {
  topic_id: string
  title?: string
  agent_id: string
  channel: string
  is_thinking?: boolean
}

// VizierRequestContent - matches backend VizierRequestContent enum with serde rename_all = "snake_case"
export type VizierRequestContent =
  | { chat: string }
  | { prompt: string }
  | { silent_read: string }
  | { task: string }
  | { command: string }
  | { reaction: ReactionEvent }

// Reaction types
export interface PlatformMessageId {
  Discord?: number
  Telegram?: number
}

export type ReactionAction = 'added' | 'removed'

export interface ReactionEvent {
  platform_message_id?: PlatformMessageId
  user_id: string
  emoji: string
  action: ReactionAction
}

export interface ReactionEntry {
  user_id: string
  emoji: string
}

// VizierResponseContent - matches backend VizierResponseContent enum with serde rename_all = "snake_case"
export type VizierResponseContent =
  | 'thinking_start'
  | { thinking: string }
  | { tool_choice: { name: string; args: Record<string, unknown> } }
  | { message: { content: string; stats?: VizierResponseStats } }
  | 'empty'
  | 'abort'

// VizierRequest as returned by backend
export interface VizierRequestMessage {
  timestamp: string
  user: string
  content: VizierRequestContent
  metadata?: Record<string, unknown>
  attachments?: VizierAttachment[]
}

// VizierAttachment - matches backend VizierAttachment
export interface VizierAttachment {
  filename: string
  content: { url: string } | { bytes: number[] } | { base64: string } | { local: string }
}

// Response stats from backend
export interface VizierResponseStats {
  input_tokens: number
  cached_input_tokens: number
  total_cached_input_tokens: number
  total_input_tokens: number
  total_output_tokens: number
  total_tokens: number
  duration: { secs: number; nanos: number }
}

export interface ChatMessage {
  uid: string
  vizier_session: {
    agent_id: string
    channel: string
    topic: string
  }
  content: {
    Request?: VizierRequestMessage
    Response?: {
      timestamp: string
      content: VizierResponseContent
      attachments?: VizierAttachment[]
    }
  }
  reactions?: ReactionEntry[]
}

export interface WebSocketMessage {
  timestamp: string
  user: string
  content: VizierRequestContent
  metadata?: Record<string, unknown>
  attachments?: VizierAttachment[]
}

// WebSocketResponse matches backend VizierResponse struct
export interface WebSocketResponse {
  timestamp: string
  content: VizierResponseContent
  attachments?: VizierAttachment[]
}

// ============================================================================
// MEMORY
// ============================================================================

export type MemoryVisibility = 'private' | 'global' | 'shared'

export interface Memory {
  agent_id: string
  slug: string
  title: string
  content?: string
  timestamp: string
  visibility: MemoryVisibility
  shared_to: string[]
  tags: string[]
  keywords: string[]
  relations: string[]
}

export interface MemoryDetail extends Memory {
  content: string
}

export interface CreateMemoryRequest {
  title: string
  content: string
  slug?: string
  visibility?: MemoryVisibility
  shared_to?: string[]
  tags?: string[]
}

export interface UpdateMemoryRequest {
  title: string
  content: string
  visibility?: MemoryVisibility
  shared_to?: string[]
  tags?: string[]
}

export interface PaginatedMemoryResponse {
  memories: Memory[]
  total: number
  offset: number
  limit: number
}

export interface MemoryGraph {
  nodes: MemoryGraphNode[]
  edges: MemoryGraphEdge[]
}

export interface MemoryGraphNode {
  slug: string
  title: string
  tags: string[]
  visibility: MemoryVisibility
  agent_id: string
}

export interface MemoryGraphEdge {
  source: string
  target: string
  broken: boolean
}

// ============================================================================
// TASK
// ============================================================================

export type TaskSchedule =
  | { CronTask: string }
  | { OneTimeTask: string }

export interface Task {
  slug: string
  user: string
  title: string
  instruction: string
  is_active: boolean
  schedule: TaskSchedule
  last_executed_at?: string
  timestamp: string
}

export interface CreateTaskRequest {
  slug: string
  user: string
  title: string
  instruction: string
  schedule: { type: 'Cron'; expression: string } | { type: 'OneTime'; datetime: string }
}

export interface UpdateTaskRequest extends CreateTaskRequest { }

// ============================================================================
// USAGE
// ============================================================================

export interface UsageSummary {
  total_tokens: number
  total_input_tokens: number
  total_output_tokens: number
  total_requests: number
  avg_duration_ms: number
}

export interface ChannelUsage {
  channel_id: string
  total_tokens: number
  total_requests: number
}

export interface ChannelTypeUsageDetail {
  total_tokens: number
  input_tokens: number
  output_tokens: number
  total_requests: number
}

export interface DailyChannelTypeUsage {
  date: string
  by_channel_type: Record<string, ChannelTypeUsageDetail>
}

export interface ChannelTypeUsage {
  total_tokens: number
  total_requests: number
  channels: ChannelUsage[]
}

export interface DailyUsage {
  date: string
  total_tokens: number
  input_tokens: number
  output_tokens: number
  total_requests: number
}

export interface AgentUsageStats {
  summary: UsageSummary
  by_channel_type: Record<string, ChannelTypeUsage>
  by_day: DailyUsage[]
  by_day_and_channel_type: DailyChannelTypeUsage[]
}

// ============================================================================
// GLOBAL CONFIG
// ============================================================================

export interface McpServerConfig {
  host: 'local' | 'http'
  command?: string
  args?: string[]
  env?: Record<string, string>
  uri?: string
}

export interface GlobalConfigEntry {
  key: string
  value: {
    type: 'McpServers' | 'Shell'
    data: Record<string, McpServerConfig> | ShellConfigData
  }
}

export interface ShellConfigData {
  environment: 'local' | 'docker'
  path?: string
  env?: Record<string, string>
  image?: { source: 'pull'; name: string } | { source: 'dockerfile'; path: string; name: string }
  container_name?: string
}

// ============================================================================
// SKILL
// ============================================================================

export type SkillActivation = 'Always' | 'OnDemand' | 'Contextual'

export interface Skill {
  name: string
  description: string
  keywords: string[]
  activation: SkillActivation
  version: number
  resources: string[]
  content?: string
  agent_id?: string
}

export interface CreateSkillRequest {
  name: string
  description: string
  content: string
  keywords?: string[]
  activation?: SkillActivation
}

export interface UpdateSkillRequest {
  description?: string
  content?: string
  keywords?: string[]
  activation?: SkillActivation
}

// ============================================================================
// SHARING
// ============================================================================

export interface SharingResponse {
  shared_to: string[]
}

export interface UpdateSharingRequest {
  add?: string[]
  remove?: string[]
}

// ============================================================================
// DREAM
// ============================================================================

export interface DreamJournalEntry {
  id: string
  dream_cycle_id: string
  agent_id: string
  timestamp: string
  stage: 'extraction' | 'consolidation'
  source_sessions: string[]
  session_context: string | null
  content: string
  duration_ms: number | null
  provider_used: string | null
  model_used: string | null
}

export interface DreamStatusResponse {
  status: 'idle' | 'extracting' | 'consolidating'
  total_sessions?: number
  completed_sessions?: number
  last_dream: string | null
  next_dream: string | null
  dream_provider: string | null
  dream_model: string | null
}
