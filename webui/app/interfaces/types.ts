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

export type IndexerKind = 'sqlite'

export interface IndexerConfig {
  kind: IndexerKind
}

export type EmbeddingProvider =
  | 'local'
  | 'openrouter'
  | 'ollama'
  | 'openai'
  | 'gemini'
  | 'voyageai'
  | 'mistral'
  | 'together'
  | 'cohere'
  | 'copilot'

export interface EmbeddingToolSettings {
  provider: EmbeddingProvider
  model: string
  base_url?: string
}

export interface LocalEmbeddingModel {
  variant: string
  name: string
  tier: 'lightweight' | 'balanced' | 'performance'
}

export const EMBEDDING_PROVIDERS: EmbeddingProvider[] = [
  'local',
  'ollama',
  'openai',
  'gemini',
  'openrouter',
  'voyageai',
  'mistral',
  'together',
  'cohere',
  'copilot',
]

export const TTS_PROVIDER_MODELS: Record<TtsProvider, string[]> = {
  openai: ['tts-1', 'tts-1-hd'],
  openrouter: [],
  elevenlabs: ['eleven_multilingual_v2', 'eleven_turbo_v2_5', 'eleven_flash_v2_5', 'eleven_monolingual_v1'],
  xai: [],
  hyperbolic: [],
  kokoro: ['kokoro-en-v0_19', 'kokoro-multi-lang-v1_0', 'kokoro-multi-lang-v1_1'],
}

export const TTS_PROVIDER_VOICES: Record<TtsProvider, string[]> = {
  openai: ['alloy', 'echo', 'fable', 'nova', 'shimmer', 'coral', 'ash', 'sage'],
  openrouter: ['alloy', 'echo', 'fable', 'nova', 'shimmer'],
  elevenlabs: [],
  xai: [],
  hyperbolic: [],
  kokoro: ['af', 'af_bella', 'af_nicole', 'af_sarah', 'af_sky', 'am_adam', 'am_michael', 'bf_emma', 'bf_isabella', 'bm_george', 'bm_lewis'],
}

export const STT_PROVIDER_MODELS: Record<SttProvider, string[]> = {
  openai: ['whisper-1'],
  elevenlabs: ['scribe_v1'],
  groq: ['whisper-large-v3', 'whisper-large-v3-turbo'],
  mistral: [],
  huggingface: [],
  gemini: [],
  whisper: ['tiny', 'tiny.en', 'base', 'base.en', 'small', 'medium', 'large-v3'],
}

export const IMAGE_GEN_PROVIDER_MODELS: Record<ImageGenProvider, string[]> = {
  openai: ['dall-e-3', 'gpt-image-1'],
  xai: ['grok-2-image'],
  huggingface: [],
  hyperbolic: ['SDXL', 'FLUX.1-dev', 'FLUX.1-schnell'],
}

export type TtsProvider =
  | 'openai'
  | 'openrouter'
  | 'elevenlabs'
  | 'xai'
  | 'hyperbolic'
  | 'kokoro'

export interface TtsToolSettings {
  provider?: TtsProvider
  model?: string
  voice?: string
  speed?: number
}

export type SttProvider =
  | 'openai'
  | 'elevenlabs'
  | 'groq'
  | 'mistral'
  | 'huggingface'
  | 'gemini'
  | 'whisper'

export interface SttToolSettings {
  provider?: SttProvider
  model?: string
  language?: string
}

export interface ReadImageToolSettings {
  provider?: string
  model?: string
}

export type ImageGenProvider = 'openai' | 'xai' | 'huggingface' | 'hyperbolic'

export interface ImageGenToolSettings {
  provider?: ImageGenProvider
  model?: string
  size?: string
}

export interface AgentToolsConfig {
  timeout: string
  shell: ShellConfigData | null
  fetch: AgentToolConfig
  http_client: AgentToolConfig
  tts: AgentToolConfig
  tts_settings?: TtsToolSettings
  stt: AgentToolConfig
  stt_settings?: SttToolSettings
  read_image: AgentToolConfig
  read_image_settings?: ReadImageToolSettings
  image_gen: AgentToolConfig
  image_gen_settings?: ImageGenToolSettings
}

export interface AgentConfig {
  name: string
  system_prompt?: string
  description?: string
  provider: string
  model: string
  thinking_depth: number
  checkpoint_threshold: number
  tools: AgentToolsConfig
  silent_read_initiative_chance: number
  max_tokens?: number
  include_documents?: string[]
  prompt_timeout: string
  dream_enabled: boolean
  dream_schedule: string | null
  dream_provider: string | null
  dream_model: string | null
  embedding?: EmbeddingToolSettings
  indexer?: IndexerConfig
}

export interface CreateAgentRequest {
  agent_id: string
  name: string
  description?: string
  provider: string
  model: string
  system_prompt?: string
  thinking_depth?: number
  checkpoint_threshold?: number
  max_tokens?: number
  context_window?: number
  silent_read_initiative_chance?: number
  tools?: {
    shell?: ShellConfigData | null
    fetch?: boolean
    http_client?: boolean
    timeout?: string
    tts?: boolean
    tts_settings?: TtsToolSettings
    stt?: boolean
    stt_settings?: SttToolSettings
    read_image?: boolean
    read_image_settings?: ReadImageToolSettings
    image_gen?: boolean
    image_gen_settings?: ImageGenToolSettings
  }
  prompt_timeout?: string
  dream_enabled?: boolean
  dream_schedule?: string
  dream_provider?: string | null
  dream_model?: string | null
  avatar_url?: string
  embedding?: EmbeddingToolSettings
  indexer?: IndexerConfig
}

export interface AgentDetail {
  agent_id: string
  name: string
  description?: string
  provider: string
  model: string
  system_prompt?: string
  thinking_depth: number
  checkpoint_threshold: number
  max_tokens?: number
  context_window?: number
  silent_read_initiative_chance?: number
  shell: ShellConfigData | null
  fetch: boolean
  http_client: boolean
  prompt_timeout: string
  dream_enabled: boolean
  dream_schedule: string | null
  dream_provider: string | null
  dream_model: string | null
  tools_timeout: string
  tts: boolean
  tts_settings?: TtsToolSettings
  stt: boolean
  stt_settings?: SttToolSettings
  read_image: boolean
  read_image_settings?: ReadImageToolSettings
  image_gen: boolean
  image_gen_settings?: ImageGenToolSettings
  avatar_url?: string
  embedding?: EmbeddingToolSettings
  indexer?: IndexerConfig
}

// ============================================================================
// PROVIDERS
// ============================================================================

/** A credential field a provider's connect form asks for. */
export type CredentialField = 'api_key' | 'endpoint' | 'base_url' | 'access_token' | 'account_id'

export interface UsedBy {
  chat: number
  stt: number
  tts: number
  image_gen: number
  embedding: number
}

/** A connected provider: saved credentials or a key found in the environment. */
export interface ProviderResponse {
  id: string
  /** Same as `id`; the field name pre-catalogue clients read. */
  variant: string
  name: string
  origin: ProviderOrigin
  kind: ProviderKind
  logo: string | null
  model_count: number
  fields: CredentialField[]
  /** `saved`, `env:NAME`, or null for a local provider on its default address. */
  source: string | null
  has_api_key: boolean
  endpoint?: string | null
  base_url?: string | null
  has_access_token: boolean
  account_id?: string | null
  migrated_from: string | null
  used_by: UsedBy
  reachable: boolean | null
}

/** Omitted keeps the stored value; an empty string clears it. */
export interface UpsertProviderRequest {
  api_key?: string
  base_url?: string
  access_token?: string
  account_id?: string
  endpoint?: string
}

export interface AffectedAgent {
  id: string
  name: string
  features: string[]
}

export interface ProviderTestResult {
  ok: boolean
  model_count?: number
  status?: number
  error?: string
}

// ============================================================================
// CATALOGUE
// ============================================================================

export type ProviderOrigin = 'catalogue' | 'builtin' | 'custom'
export type ProviderKind = 'cloud' | 'local' | 'media'
export type DialectKind = 'open_ai_chat' | 'open_ai_responses' | 'anthropic' | 'gemini' | 'ollama'

export interface CatalogueStatus {
  source: 'bundled' | 'cached'
  fetched_at: string
  checked_at: string | null
  refresh: {
    enabled: boolean
    ttl: string
    in_progress: boolean
    last_error: string | null
    failed_at: string | null
  }
  provider_count: number
  model_count: number
}

export interface ProviderConnection {
  status: 'connected' | 'missing' | 'unreachable'
  /** `saved`, `env:NAME`, or null. Never a secret. */
  source: string | null
}

export interface CatalogueProvider {
  id: string
  name: string
  origin: ProviderOrigin
  kind: ProviderKind
  dialect: DialectKind | null
  logo: string | null
  logo_full_color: boolean
  model_count: number
  connection: ProviderConnection
  env: string[]
  fields: CredentialField[]
  popular: boolean
  /** Where requests go by default; may hold `${VAR}`. */
  endpoint: string | null
  doc: string | null
}

export interface ModelLimits {
  context: number | null
  input: number | null
  output: number | null
}

export interface ModelCaps {
  tool_call: boolean | null
  reasoning: boolean | null
  image_input: boolean | null
  attachment: boolean | null
}

/** USD per million tokens. */
export interface ModelCost {
  input: number
  output: number
  cache_read: number | null
  cache_write: number | null
}

export interface CatalogueModel {
  id: string
  name: string
  default: boolean
  logo: string | null
  logo_full_color: boolean
  limit: ModelLimits
  caps: ModelCaps
  cost: ModelCost | null
  status: 'active' | 'beta' | 'alpha' | 'deprecated'
  release_date: string | null
  knowledge: string | null
  /** `server`: a local provider's live model the catalogue doesn't have. */
  origin?: string
  /** Custom providers: fields filled in from the catalogue. */
  filled_from_catalogue?: string[]
}

export interface AgentWarning {
  code: 'no_tool_calling' | 'no_image_input' | 'unknown_model'
  message: string
}

// ============================================================================
// CUSTOM PROVIDERS
// ============================================================================

export type CustomProtocol = 'openai_compatible' | 'anthropic'

export interface CustomModel {
  id: string
  context?: number | null
  output?: number | null
  tool_call?: boolean | null
  image_input?: boolean | null
  reasoning?: boolean | null
  edited?: boolean
}

export interface CustomProviderResponse {
  id: string
  name: string
  protocol: CustomProtocol
  base_url: string
  has_api_key: boolean
  /** Header names only. */
  headers: string[]
  models: CustomModel[]
  models_fetched_at: string | null
  used_by: UsedBy
  reachable: boolean | null
}

export interface CustomProviderBody {
  id?: string
  name: string
  protocol: CustomProtocol
  base_url: string
  /** Omitted keeps the stored key; '' clears it. */
  api_key?: string
  /** An omitted header keeps its value; null removes it. */
  headers?: Record<string, string | null>
  models?: CustomModel[]
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
  // Background jobs launched from this topic that are still in flight.
  running_jobs?: number
}

// ============================================================================
// BACKGROUND JOBS (specs/012-background-subagent-results/contracts/http-api.md)
// ============================================================================

export type BackgroundJobKind = 'batch' | 'delegation'

// `running` and `reporting` are in flight.
export type BackgroundJobState =
  | 'running'
  | 'reporting'
  | 'reported'
  | 'undelivered'
  | 'cancelled'
  | 'interrupted'

export type BackgroundPieceState =
  | 'running'
  | 'answered'
  | 'failed'
  | 'timed_out'
  | 'cancelled'
  | 'interrupted'

export type BackgroundCanceller = { agent: string } | { person: string }

export interface BackgroundPieceSnapshot {
  ordinal: number
  prompt: string
  state: BackgroundPieceState
  reason: string | null
  agent_id: string
  topic: string | null
  started_at: string
  finished_at: string | null
}

export interface BackgroundJobSnapshot {
  id: string
  kind: BackgroundJobKind
  delegated_to: string | null
  state: BackgroundJobState
  created_at: string
  finished_at: string | null
  timeout_secs: number
  cancelled_by?: BackgroundCanceller | null
  reason?: string | null
  pieces: BackgroundPieceSnapshot[]
}

export interface ReportEntry {
  ordinal: number
  prompt: string
  state: BackgroundPieceState
  text: string
  truncated: boolean
}

export interface BackgroundReport {
  job_id: string
  kind: BackgroundJobKind
  delegated_to: string | null
  entries: ReportEntry[]
}

// A WebSocket frame announcing a background job change. Every other frame is a bare
// `WebSocketResponse`, which has no top-level `background_job` key.
export interface WebSocketJobFrame {
  background_job: BackgroundJobSnapshot
}

// VizierRequestContent - matches backend VizierRequestContent enum with serde rename_all = "snake_case"
export type VizierRequestContent =
  | { chat: string }
  | { prompt: string }
  | { silent_read: string }
  // A machine wrote this prompt and nobody is waiting on the answer — a scheduled task run,
  // or a dream cycle's own work. Renamed from `task`, because two of its three construction
  // sites are the dream cycle.
  | { unattended: string }
  | { command: string }
  | { audio_chat: [VizierAttachment, string | null] }
  | { audio_prompt: [VizierAttachment, string | null] }
  // The outcome of background work this conversation started. Not a message from a person.
  | { background_report: BackgroundReport }

// Reaction types
export type ReactionAction = 'added' | 'removed'

export interface ReactionEntry {
  user_id: string
  emoji: string
  // Display name at the time of reacting, when the channel resolved one. Unused by the WebUI.
  user_name?: string
}

// Client → server: change one of the person's own reactions on an agent reply.
export interface WebSocketReactionMessage {
  reaction: { message_uid: string; emoji: string; action: ReactionAction }
}

// Server → every socket on the session: the full current set of one message, after any
// change to it. The sender's own socket gets it too, as its acknowledgement.
export interface WebSocketReactionsFrame {
  reactions: { message_uid: string; reactions: ReactionEntry[] }
}

// Server → the sending socket only: a reaction that was not saved.
export interface WebSocketReactionErrorFrame {
  reaction_error: { message_uid: string; message: string }
}

// Python sandbox execution report (payload of a `tool_response` event for execute_python)
export interface ToolInvocationRecord {
  seq: number
  name: string
  // Present in the session record; dropped from the copy sent to the model.
  arguments?: Record<string, unknown>
  ok: boolean
  error?: string | null
  duration_ms: number
}

export interface ExecutionError {
  kind: 'script' | 'tool' | 'limit'
  message: string
  // Omitted by the server for host-raised limits, which have no traceback.
  traceback?: string
  limit?: string | null
}

export interface ExecutionReport {
  ok: boolean
  result: unknown
  stdout: string
  error?: ExecutionError | null
  tool_calls: ToolInvocationRecord[]
  duration_ms: number
}

// VizierResponseContent - matches backend VizierResponseContent enum with serde rename_all = "snake_case"
export type VizierResponseContent =
  | 'thinking_start'
  | { thinking: string }
  | { tool_choice: { name: string; args: Record<string, unknown> } }
  | { tool_response: { response: unknown } }
  | { message: { content: string; stats?: VizierResponseStats } }
  | { audio_reply: [VizierAttachment, string | null, VizierResponseStats | null] }
  | { error: { kind: 'completion' | 'tool_timeout' | 'prompt_timeout' | 'rate_limited' | 'provider_unavailable' | 'agent_unavailable'; message: string } }
  | 'empty'
  | 'abort'
  | { checkpoint: { handover: string | null } }

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
  cache_creation_input_tokens: number
  total_cache_creation_input_tokens: number
  current_context_size?: number
  context_window?: number
}

export interface ChatMessage {
  uid: string
  timestamp?: string
  // Explicit ordering position, assigned by storage on write. Null for entries recorded
  // before the column existed, which are not retroactively ordered.
  seq?: number | null
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
    // Intermediate narration: text the agent wrote alongside its tool calls.
    AssistantMessage?: string
    ToolCall?: { call_id: string; name: string; arguments: Record<string, unknown> }
    ToolResult?: { call_id: string; content: string }
    Checkpoint?: string | { handover: string | null; timestamp: string }
    Command?: string
  }
  reactions?: ReactionEntry[]
  // False for a live reply that arrived without the server's history uid (an older server):
  // a reaction on it could not be saved, so it gets no react control.
  serverUid?: boolean
}

// ============================================================================
// ACTIVITY TRAIL
// ============================================================================

// One entry of what an agent did on its way to an answer, normalized so the live
// WebSocket stream and stored history produce the same shape and render through the same
// component. There is deliberately no variant for a tool call made by a python script:
// nested calls never reach history and are not streamed, so neither producer can emit one.
export type TrailEvent =
  | { kind: 'narration'; id: string; text: string }
  | { kind: 'thought'; id: string; text: string }
  // `jobId` is set on the `paralel_subtasks`/`delegate_agent` call that launched a background
  // job, read from its tool result, so the job tray can jump back to it.
  | { kind: 'tool'; id: string; name: string; args: Record<string, unknown>; jobId?: string }
  | {
      kind: 'python'
      id: string
      intent: string | null
      code: string | null
      report: ExecutionReport | null
    }

// One exchange: what was asked, what the agent did on the way, and what came back.
export interface Turn {
  key: string
  // Absent for an agent-initiated turn — a scheduled task or a dream cycle.
  request?: ChatMessage
  trail: TrailEvent[]
  outcome?: ChatMessage
  // The uid of the entry the turn ends at — its outcome, or the checkpoint or command that
  // closed it. It is what the trail renders above, and it is what lets a turn closed live
  // and the same turn reloaded from history put their trail in the same place. Absent for a
  // turn that nothing closed, such as one interrupted by a reload.
  anchorUid?: string
  // True while the turn is still streaming, which is when its trail renders expanded.
  live: boolean
}

export interface WebSocketMessage {
  timestamp: string
  user: string
  content: VizierRequestContent
  metadata?: Record<string, unknown>
  attachments?: VizierAttachment[]
  expect_audio_reply?: boolean
}

// WebSocketResponse matches backend VizierResponse struct
export interface WebSocketResponse {
  timestamp: string
  content: VizierResponseContent
  attachments?: VizierAttachment[]
  // The history uid the final frame of a turn was saved as; absent on every other frame.
  history_uid?: string
}

// ============================================================================
// MEMORY
// ============================================================================

export interface Memory {
  agent_id: string
  bundle: string
  path: string
  title: string
  content?: string
  created_at: string
  updated_at: string
  tags: string[]
  relations: string[]
  attachment_count: number
}

export interface MemoryDetail extends Omit<Memory, 'attachment_count'> {
  content: string
  attachments: VizierAttachment[]
}

export interface CreateMemoryRequest {
  title: string
  content: string
  bundle?: string
  path?: string
  tags?: string[]
  attachments?: VizierAttachment[]
}

export interface UpdateMemoryRequest {
  title: string
  content: string
  tags?: string[]
  attachments?: VizierAttachment[]
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
  initial_slugs: string[]
}

export interface MemoryGraphNode {
  slug: string
  bundle: string
  title: string
  tags: string[]
  agent_id: string
  boundary: boolean
}

export interface MemoryGraphEdge {
  source: string
  target: string
  broken: boolean
}

export interface BundleSummary {
  name: string
  concept_count: number
  updated_at: string | null
}

export interface ImportReport {
  imported: string[]
  skipped: string[]
}

// ============================================================================
// TASK
// ============================================================================

export type TaskSchedule =
  | { CronTask: string }
  | { OneTimeTask: string }

// Who wanted the task to exist: somebody asked, or the agent set it up on its own
// initiative. A person is recorded as the channel they reached the agent on knows them, so
// it may not resolve to any account — it renders as recorded either way.
export type Requester =
  | { user: string }
  | { agent: string }

export type TaskRunState = 'running' | 'answered' | 'no_response' | 'interrupted'

// One firing of a task. `response` is present only where the response is what the reader
// came for — the task detail and a run's own fetch. The listing and the task table leave it
// out entirely.
export interface TaskRun {
  run_id: string
  id: number
  ran_at: string
  finished_at?: string | null
  state: TaskRunState
  response?: string | null
}

export interface TaskRunsResponse {
  runs: TaskRun[]
  has_more: boolean
}

export interface Task {
  slug: string
  requester: Requester
  title: string
  instruction: string
  is_active: boolean
  schedule: TaskSchedule
  last_executed_at?: string
  timestamp: string
  // `null` for a task that has never run — distinct from a run that answered nothing, which
  // is a `last_run` whose `response` is `null`.
  last_run?: TaskRun | null
}

// `user` is gone: the requester is taken from the authenticated caller, never supplied.
export interface CreateTaskRequest {
  slug: string
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
  /** Null when no reply in range has a known price. */
  estimated_cost_usd: number | null
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
  estimated_cost_usd: number | null
}

export interface DailyUsage {
  date: string
  total_tokens: number
  input_tokens: number
  output_tokens: number
  total_requests: number
  estimated_cost_usd: number | null
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

export interface GlobalConfigEntry {
  key: string
  value: {
    type: 'Shell'
    data: ShellConfigData
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

export interface Skill {
  name: string
  description: string
  keywords: string[]
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
}

export interface UpdateSkillRequest {
  description?: string
  content?: string
  keywords?: string[]
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

// ============================================================================
// VERSION HISTORY (CORE.md + memories) — specs/006-memory-version-history
// ============================================================================

export type RevisionActor =
  | { type: 'agent' }
  | { type: 'user'; user_id: string; username: string }
  | { type: 'system' }

export type RevisionTrigger =
  | { type: 'conversation' }
  | { type: 'dream' }
  | { type: 'webui' }
  | { type: 'api' }
  | { type: 'import' }
  | { type: 'rollback'; restored_from: number }
  | { type: 'baseline' }

export interface CoreRevisionSummary {
  seq: number
  is_current: boolean
  size_bytes: number
  actor: RevisionActor
  trigger: RevisionTrigger
  created_at: string
}

export interface CoreRevision extends CoreRevisionSummary {
  content: string
}

export interface PaginatedCoreRevisions {
  revisions: CoreRevisionSummary[]
  total: number
  offset: number
  limit: number
}

export interface MemoryRevisionSummary extends CoreRevisionSummary {
  deleted: boolean
}

export interface MemoryRevision extends MemoryRevisionSummary {
  content: string | null
  title: string | null
  tags: string[]
}

export interface PaginatedMemoryRevisions {
  revisions: MemoryRevisionSummary[]
  total: number
  offset: number
  limit: number
}

export interface DiffLine {
  op: 'equal' | 'insert' | 'delete'
  old_line: number | null
  new_line: number | null
  text: string
}

export interface DiffHunk {
  old_start: number
  old_lines: number
  new_start: number
  new_lines: number
  lines: DiffLine[]
}

export interface RevisionDiff {
  from_seq: number
  to_seq: number
  additions: number
  deletions: number
  hunks: DiffHunk[]
}

export interface RollbackResponse {
  no_change: boolean
  new_seq: number | null
  restored_from: number
}

// Normalized row shape the shared `VersionHistory` panel renders; each route maps its own API
// type into it (CORE rows get `deleted: false`, no title/tags).
export interface HistoryRow {
  seq: number
  is_current: boolean
  deleted: boolean
  size_bytes: number
  actor: RevisionActor
  trigger: RevisionTrigger
  created_at: string
}

export interface HistoryVersion extends HistoryRow {
  content: string | null
  title?: string | null
  tags?: string[]
}

// ============================================================================
// INTEGRATIONS (specs 016, 017)
// ============================================================================

export type IntegrationFamily = 'channel' | 'addon' | 'mcp'

export type IntegrationStatus =
  | { state: 'disabled' }
  | { state: 'incomplete'; missing: string[] }
  | { state: 'connecting' }
  | { state: 'running'; tools?: number }
  | { state: 'reconnecting'; last_error: string; next_retry_at?: string }
  | { state: 'failed'; last_error: string }
  | { state: 'unreachable'; last_error: string }
  | { state: 'unknown' }

/** An integration as every response shows it; secrets come back as `••••<last 4>`. */
export interface Integration {
  id: string
  kind: string
  family: IntegrationFamily
  name: string
  enabled: boolean
  /** Who made it, a person or the agent itself. Informational only. */
  creator: { user: string } | { agent: string } | null
  /** The agent's name, or the user id. */
  creator_name?: string | null
  settings: Record<string, unknown>
  missing?: string[]
}

export interface AgentRef {
  agent_id: string
  name: string
}

/** One of the agent's integrations as its Integrations page shows it. */
export interface IntegrationItem extends Integration {
  /** Set only on a row left over from before integrations were per-agent (spec 017 FR-023). */
  legacy_linked?: { agents: AgentRef[]; others: number }
  status: IntegrationStatus
  can_edit: boolean
}

export interface IntegrationList {
  family: IntegrationFamily | null
  can_manage: boolean
  items: IntegrationItem[]
}

export interface KindField {
  key: string
  label: string
  type: 'secret' | 'string' | 'bool' | 'string_list' | 'secret_map' | 'select'
  required?: boolean
  default?: unknown
  options?: string[]
}

export interface KindListing {
  id: string
  family: IntegrationFamily
  name: string
  description: string
  icon: string
  fields: KindField[]
}

export interface CreateIntegrationRequest {
  kind: string
  name: string
  settings: Record<string, unknown>
  enabled?: boolean
}

export interface UpdateIntegrationRequest {
  name?: string
  settings?: Record<string, unknown>
  enabled?: boolean
}

export type GrantFamily = 'mcp' | 'addon' | 'channel'

export interface GrantState {
  on: boolean
  granted_by?: string
  granted_at?: string
}

/** Which integration families an agent may manage itself (spec 017). */
export interface IntegrationGrants {
  can_manage: boolean
  grants: Record<GrantFamily, GrantState>
}

export type IntegrationEventAction =
  | 'created'
  | 'edited'
  | 'enabled'
  | 'disabled'
  | 'deleted'
  | 'unlinked'
  | 'grant_on'
  | 'grant_off'

export interface IntegrationEvent {
  id: string
  agent_id: string
  integration_id: string | null
  integration_name: string
  family: IntegrationFamily
  actor: { user: string } | { agent: string }
  actor_name: string
  action: IntegrationEventAction
  fields: string[]
  session: string | null
  at: string
}

export interface IntegrationEventPage {
  items: IntegrationEvent[]
  next?: string
}
