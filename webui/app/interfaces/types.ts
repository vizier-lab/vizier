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

export type ChatProvider =
  | 'mistralrs'
  | 'ollama'
  | 'openai'
  | 'anthropic'
  | 'openrouter'
  | 'gemini'
  | 'deepseek'
  | 'mimo'
  | 'llama_cpp'
  | 'groq'
  | 'mistral'
  | 'xai'
  | 'perplexity'
  | 'moonshot'
  | 'zai'
  | 'minimax'
  | 'together'
  | 'cohere'
  | 'huggingface'
  | 'hyperbolic'
  | 'galadriel'
  | 'mira'
  | 'chatgpt'
  | 'copilot'
  | 'azure'

export const CHAT_PROVIDERS: ChatProvider[] = [
  'mistralrs',
  'ollama',
  'openai',
  'anthropic',
  'openrouter',
  'gemini',
  'deepseek',
  'mimo',
  'llama_cpp',
  'groq',
  'mistral',
  'xai',
  'perplexity',
  'moonshot',
  'zai',
  'minimax',
  'together',
  'cohere',
  'huggingface',
  'hyperbolic',
  'galadriel',
  'mira',
  'chatgpt',
  'copilot',
  'azure',
]

export const CHAT_PROVIDER_DEFAULT_MODELS: Record<ChatProvider, string> = {
  mistralrs: 'google/gemma-4-E4B-it',
  ollama: 'qwen3.5:4b',
  openai: 'gpt-4o-mini',
  anthropic: 'claude-3-haiku-20240307',
  openrouter: 'anthropic/claude-3-haiku',
  gemini: 'gemini-2.0-flash',
  deepseek: 'deepseek-chat',
  mimo: 'mimo-v2.5-pro',
  llama_cpp: 'google_gemma-4-E4B-it-Q4_K_M',
  groq: 'llama-3.1-70b-versatile',
  mistral: 'mistral-large-latest',
  xai: 'grok-2-latest',
  perplexity: 'llama-3.1-sonar-large-128k-online',
  moonshot: 'moonshot-v1-128k',
  zai: 'glm-4-plus',
  minimax: 'MiniMax-Text-01',
  together: 'meta-llama/Llama-3-70b-chat-hf',
  cohere: 'command-r-plus',
  huggingface: 'meta-llama/Llama-3-70b-chat-hf',
  hyperbolic: 'meta-llama/Llama-3-70b-chat-hf',
  galadriel: 'llama3.1:70b',
  mira: 'mira-70b',
  chatgpt: 'gpt-4o',
  copilot: 'gpt-4o',
  azure: 'gpt-4o',
}

export const CHAT_PROVIDER_MODELS: Record<ChatProvider, string[]> = {
  mistralrs: [],
  ollama: ['qwen3.5:4b', 'qwen3:8b', 'qwen3:14b', 'llama3.2:3b', 'llama3.2:1b', 'llama3.1:8b', 'llama3.1:70b', 'llama3.1:405b', 'mistral:7b', 'codellama:7b', 'phi4:14b', 'phi4-mini:3.8b', 'deepseek-r1:8b', 'deepseek-r1:14b', 'gemma3:4b', 'gemma3:12b', 'nemotron-mini:4b'],
  openai: ['gpt-4o-mini', 'gpt-4o', 'gpt-4.1', 'gpt-4.1-mini', 'gpt-4.1-nano', 'o3', 'o3-mini', 'o4-mini', 'o4-mini-high', 'gpt-4.5-preview'],
  anthropic: ['claude-sonnet-4-5', 'claude-haiku-4-5', 'claude-opus-4-5', 'claude-3-5-sonnet-latest', 'claude-3-haiku-20240307', 'claude-3-5-haiku-latest'],
  openrouter: ['anthropic/claude-sonnet-4-5', 'anthropic/claude-3-haiku', 'openai/gpt-4o-mini', 'openai/gpt-4o', 'google/gemini-2.0-flash', 'google/gemini-2.5-flash', 'google/gemma-4-E4B-it', 'meta-llama/llama-3.1-8b-instruct', 'meta-llama/llama-3.1-70b-instruct', 'mistralai/mistral-7b-instruct', 'qwen/qwen3-8b', 'deepseek/deepseek-r1', 'deepseek/deepseek-chat'],
  gemini: ['gemini-2.0-flash', 'gemini-2.0-flash-lite', 'gemini-2.5-flash', 'gemini-2.5-flash-preview', 'gemini-2.5-pro'],
  deepseek: ['deepseek-chat', 'deepseek-reasoner'],
  mimo: ['mimo-v2.5-pro', 'mimo-v2.5'],
  llama_cpp: [],
  groq: ['llama-3.1-70b-versatile', 'llama-3.1-8b-instant', 'llama-3.3-70b-versatile', 'mixtral-8x7b-32768', 'gemma2-9b-it', 'deepseek-r1-distill-llama-70b', 'qwen-2.5-32b'],
  mistral: ['mistral-large-latest', 'mistral-small-latest', 'mistral-medium-latest', 'codestral-latest', 'pixtral-large-latest', 'mistral-embed'],
  xai: ['grok-2-latest', 'grok-2-vision-latest'],
  perplexity: ['llama-3.1-sonar-large-128k-online', 'llama-3.1-sonar-small-128k-online', 'llama-3.1-sonar-huge-128k-online'],
  moonshot: ['moonshot-v1-128k', 'moonshot-v1-32k', 'moonshot-v1-8k'],
  zai: ['glm-4-plus', 'glm-4-air', 'glm-4-flash', 'glm-4-long'],
  minimax: ['MiniMax-Text-01', 'MiniMax-Text-01-Latest', 'MiniMax-M1-0306'],
  together: ['meta-llama/Llama-3-70b-chat-hf', 'meta-llama/Llama-3.3-70b-Instruct-Turbo', 'mistralai/Mixtral-8x7B-Instruct-v0.1', 'Qwen/Qwen3-8B', 'Qwen/Qwen3-30B'],
  cohere: ['command-r-plus', 'command-r', 'command-light', 'command-nightly', 'command-r7b'],
  huggingface: [],
  hyperbolic: ['meta-llama/Llama-3-70b-chat-hf', 'meta-llama/Llama-3.1-8B-Instruct', 'google/gemma-4-E4B-it', 'Qwen/Qwen3-8B'],
  galadriel: ['llama3.1:70b', 'llama3.1:8b', 'mistral:7b', 'qwen2:7b'],
  mira: ['mira-70b', 'mira-8b'],
  chatgpt: ['gpt-4o', 'gpt-4o-mini', 'gpt-4.1', 'gpt-4.1-mini', 'o3', 'o4-mini'],
  copilot: ['gpt-4o', 'gpt-4o-mini', 'gpt-4.1', 'gpt-4.1-mini'],
  azure: ['gpt-4o', 'gpt-4o-mini', 'gpt-4.1', 'gpt-4.1-mini', 'o3-mini'],
}

export const TTS_PROVIDER_MODELS: Record<TtsProvider, string[]> = {
  piper: [
    'en_US-lessac-medium', 'en_US-lessac-high', 'en_US-amy-medium',
    'en_US-amy-low', 'en_US-joe-medium', 'en_US-ryan-medium',
    'en_US-ryan-high', 'en_US-kathleen-low', 'en_US-libritts_r-medium',
    'en_US-libritts-high',
    'en_GB-alan-medium', 'en_GB-southern_english_female-low',
    'en_GB-jenny_dioco-medium', 'en_GB-semaine-medium', 'en_GB-vctk-medium',
    'de_DE-thorsten-medium', 'zh_CN-huayan-medium',
    'es_ES-davefx-medium', 'fr_FR-siwis-medium', 'ru_RU-irina-medium',
  ],
  kitten: [
    'kitten-nano-en-v0_1-fp16', 'kitten-nano-en-v0_2-fp16',
    'kitten-mini-en-v0_1-fp16', 'kitten-micro-en-v0_8',
    'kitten-nano-en-v0_8-int8', 'kitten-nano-en-v0_8-fp32',
    'kitten-mini-en-v0_8',
  ],
  kokoro: ['kokoro-en-v0_19', 'kokoro-multi-lang-v1_0'],
  openai: ['tts-1', 'tts-1-hd'],
  openrouter: [],
  elevenlabs: ['eleven_multilingual_v2', 'eleven_turbo_v2_5', 'eleven_flash_v2_5', 'eleven_monolingual_v1'],
  xai: [],
  hyperbolic: [],
}

export const TTS_PROVIDER_VOICES: Record<TtsProvider, string[]> = {
  piper: [],
  kitten: ['0', '1', '2', '3', '4', '5', '6', '7'],
  kokoro: [
    'af_alloy', 'af_aoede', 'af_bella', 'af_heart', 'af_jessica',
    'af_kore', 'af_nicole', 'af_nova', 'af_river', 'af_sarah', 'af_sky',
    'am_adam', 'am_echo', 'am_eric', 'am_fenrir', 'am_liam',
    'am_michael', 'am_onyx', 'am_puck', 'am_santa',
    'bf_alice', 'bf_emma', 'bf_isabella', 'bf_lily',
    'bm_daniel', 'bm_fable', 'bm_george', 'bm_lewis',
  ],
  openai: ['alloy', 'echo', 'fable', 'nova', 'shimmer', 'coral', 'ash', 'sage'],
  openrouter: ['alloy', 'echo', 'fable', 'nova', 'shimmer'],
  elevenlabs: [],
  xai: [],
  hyperbolic: [],
}

export const STT_PROVIDER_MODELS: Record<SttProvider, string[]> = {
  sense_voice: [
    'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17',
    'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17',
    'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09',
    'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2025-09-09',
    'sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17',
    'sherpa-onnx-sense-voice-funasr-nano-2025-12-17',
  ],
  openai: ['whisper-1'],
  elevenlabs: ['scribe_v1'],
  groq: ['whisper-large-v3', 'whisper-large-v3-turbo'],
  mistral: [],
  huggingface: [],
  gemini: [],
}

export const IMAGE_GEN_PROVIDER_MODELS: Record<ImageGenProvider, string[]> = {
  openai: ['dall-e-3', 'gpt-image-1'],
  xai: ['grok-2-image'],
  huggingface: [],
  hyperbolic: ['SDXL', 'FLUX.1-dev', 'FLUX.1-schnell'],
}

export interface BraveSearchToolSettings {
  api_key?: string
  safesearch?: boolean
}

export type TtsProvider =
  | 'piper'
  | 'kitten'
  | 'kokoro'
  | 'openai'
  | 'openrouter'
  | 'elevenlabs'
  | 'xai'
  | 'hyperbolic'

export interface TtsToolSettings {
  provider?: TtsProvider
  model?: string
  voice?: string
  speed?: number
}

export type SttProvider =
  | 'sense_voice'
  | 'openai'
  | 'elevenlabs'
  | 'groq'
  | 'mistral'
  | 'huggingface'
  | 'gemini'

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
  brave_search: AgentToolConfig
  brave_search_settings?: BraveSearchToolSettings
  discord: AgentToolConfig
  telegram: AgentToolConfig
  fetch: AgentToolConfig
  http_client: AgentToolConfig
  mcp_servers: Record<string, McpServerConfig>
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
  embedding?: EmbeddingToolSettings
  indexer?: IndexerConfig
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
  checkpoint_threshold?: number
  max_tokens?: number
  context_window?: number
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
    timeout?: string
    mcp_servers?: Record<string, McpServerConfig>
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
  heartbeat_interval?: string
  dream_enabled?: boolean
  dream_schedule?: string
  dream_provider?: string | null
  dream_model?: string | null
  discord_token?: string
  telegram_token?: string
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
  quantization?: string
  system_prompt?: string
  thinking_depth: number
  checkpoint_threshold: number
  max_tokens?: number
  context_window?: number
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
  access_token?: string
  account_id?: string
  endpoint?: string
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
  | { audio_chat: [VizierAttachment, string | null] }
  | { audio_prompt: [VizierAttachment, string | null] }

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
  | { audio_reply: [VizierAttachment, string | null, VizierResponseStats | null] }
  | { error: { kind: 'completion' | 'tool_timeout' | 'prompt_timeout'; message: string } }
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
    Checkpoint?: string | { handover: string | null; timestamp: string }
    Command?: string
  }
  reactions?: ReactionEntry[]
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
  attachments: VizierAttachment[]
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
  attachments?: VizierAttachment[]
}

export interface UpdateMemoryRequest {
  title: string
  content: string
  visibility?: MemoryVisibility
  shared_to?: string[]
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
