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

// VizierRequestContent can be one of: Chat(string), Prompt(string), Task(string), Command(string), SilentRead(string)
export interface VizierRequestContent {
  Chat?: string
  Prompt?: string
  Task?: string
  Command?: string
  SilentRead?: string
}

// VizierRequest as returned by backend
export interface VizierRequestMessage {
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
    Response?: [string, VizierResponseStats | null] | null
  }
  timestamp: string
}

export interface WebSocketMessage {
  user: string
  content: { Chat: string } | { Prompt: string } | { Task: string } | { Command: string }
  metadata?: Record<string, unknown>
}

export type WebSocketResponse =
  | { ThinkingStart: null }
  | { 
      ToolChoice: {
        name: string
        args: Record<string, unknown>
      }
    }
  | { Thinking: string }
  | {
      Message: {
        content: string
        stats?: {
          input_tokens: number
          cached_input_tokens: number
          total_cached_input_tokens: number
          total_input_tokens: number
          total_output_tokens: number
          total_tokens: number
          duration: { secs: number; nanos: number }
        }
      }
    }
  | { Empty: null }
  | { Abort: null }

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
