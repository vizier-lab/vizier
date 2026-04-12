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

export interface ApiKey {
  id: string
  name: string
  key?: string // Only returned on creation
  expires_at?: string
  created_at: string
  last_used_at?: string
}

// ============================================================================
// AGENT
// ============================================================================

export interface Agent {
  agent_id: string
  name: string
  description?: string
}

// ============================================================================
// CHAT/TOPIC
// ============================================================================

export interface Topic {
  topic_id: string
  title?: string
  agent_id: string
  channel: string
}

// VizierRequestContent - matches backend VizierRequestContent enum with serde rename_all = "snake_case"
export type VizierRequestContent =
  | { chat: string }
  | { prompt: string }
  | { silent_read: string }
  | { task: string }
  | { command: string }

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
    }
  }
}

export interface WebSocketMessage {
  timestamp: string
  user: string
  content: VizierRequestContent
  metadata?: Record<string, unknown>
}

// WebSocketResponse matches backend VizierResponse struct
export interface WebSocketResponse {
  timestamp: string
  content: VizierResponseContent
}

// ============================================================================
// MEMORY
// ============================================================================

export interface Memory {
  agent_id: string
  slug: string
  title: string
  content?: string
  timestamp: string
}

export interface MemoryDetail extends Memory {
  content: string
}

export interface CreateMemoryRequest {
  title: string
  content: string
  slug?: string
}

export interface UpdateMemoryRequest {
  title: string
  content: string
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

export interface UpdateTaskRequest extends CreateTaskRequest {}

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
