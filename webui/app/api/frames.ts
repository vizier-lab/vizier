// The wire types of a chat topic: requests, replies, history entries, jobs, client tools and
// the frames the socket carries (specs/024-webui-rewrite/contracts/websocket.md). Hand-typed:
// the socket protocol is not in the OpenAPI document. Carried over from the previous WebUI,
// plus the `Recall` entry and the `session_file` frame.


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
  // A message a client sent on the person's behalf, e.g. a click in a page the agent drew.
  // `text` is the person's bubble; `data` is for the agent only and is never shown.
  | { client_event: ClientEventContent }

export interface ClientEventContent {
  text: string
  data?: unknown
  source_call_id?: string
}

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
  // The provider and model that answered.
  provider?: string
  model?: string
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
      // The turn was interrupted by a restart and carried on afterwards.
      resumed?: boolean
    }
    // Intermediate narration: text the agent wrote alongside its tool calls.
    AssistantMessage?: string
    ToolCall?: { call_id: string; name: string; arguments: Record<string, unknown> }
    ToolResult?: { call_id: string; content: string }
    Checkpoint?: string | { handover: string | null; timestamp: string }
    Command?: string
    // Memory passages the turn was given or searched for (spec 024, FR-018e). Display only.
    Recall?: { origin: RecallOrigin; passages: RecalledPassage[] }
    // A call the agent made to a tool this client (or another) declared. The entry's uid is
    // the call id; `outcome` and `state` are filled in by the server on every read.
    ClientToolCall?: ClientToolCallEntry
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
  // `jobId` is set on the `background_subagents`/`delegate_agent` call that launched a
  // background job, read from its tool result, so the job tray can jump back to it. The
  // retired `paralel_subtasks` name is kept for saved history.
  | { kind: 'tool'; id: string; name: string; args: Record<string, unknown>; jobId?: string; stopped?: boolean }
  | {
      kind: 'python'
      id: string
      intent: string | null
      code: string | null
      report: ExecutionReport | null
      stopped?: boolean
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
  // The client tool calls the turn made, in order. Drawn inside the turn's reply bubble.
  clientCalls: ClientCallView[]
  // True when the person aborted the turn: no outcome, closed by an `abort` command.
  aborted?: boolean
}

// A step of a running turn, pushed as it is saved (specs/023-persist-turn-activity).
export interface HistoryEntryFrame {
  history_entry: ChatMessage
}

export interface WebSocketMessage {
  timestamp: string
  user: string
  content: VizierRequestContent
  metadata?: Record<string, unknown>
  attachments?: VizierAttachment[]
  expect_audio_reply?: boolean
  // This client's tools for the turn this request starts (client tools protocol §1).
  client_tools?: ClientToolDeclaration[]
  client_categories?: ClientCategoryDeclaration[]
}

// ============================================================================
// CLIENT TOOLS (specs/019-client-tools/contracts/client-tools-protocol.md)
// ============================================================================

export type JsonSchemaObject = { type: 'object'; [key: string]: unknown }

export interface ClientToolDeclaration {
  name: string
  category?: string
  description: string
  input_schema: JsonSchemaObject
  output_schema?: Record<string, unknown>
  timeout_secs?: number
  // Keep a call made while no client is connected, and run it when one reconnects.
  deferrable?: boolean
}

export interface ClientCategoryDeclaration {
  name: string
  description?: string
}

export type ClientCallOutcome =
  | { status: 'pending' }
  | { status: 'ok'; result: unknown }
  | { status: 'error'; message: string }
  | { status: 'disconnected' }
  | { status: 'timeout' }
  | { status: 'cancelled' }
  | { status: 'interrupted' }
  // Waiting for a client to reconnect (deferrable tools only).
  | { status: 'deferred' }

export interface ClientToolCallEntry {
  name: string
  category: string
  tool: string
  arguments: Record<string, unknown>
  outcome?: ClientCallOutcome
  state?: unknown
}

/** One client tool call as a turn draws it, live or from history. */
export interface ClientCallView {
  callId: string
  name: string
  category: string
  tool: string
  args: Record<string, unknown>
  outcome?: ClientCallOutcome
  state?: unknown
  live: boolean
}

export interface ClientToolCallFrame {
  client_tool_call: {
    call_id: string
    name: string
    category: string
    tool: string
    arguments: Record<string, unknown>
    // True only on the connection that declared the tool, which alone may answer.
    answer: boolean
    // A deferred call delivered now that a client is back.
    deferred?: boolean
  }
}

export interface ClientToolsResolvedFrame {
  client_tools_resolved: {
    protocol: number
    tools: { category: string; tool: string; category_name: string; name: string }[]
  }
}

export interface ClientToolOutcomeFrame {
  client_tool_outcome: { call_id: string; outcome: ClientCallOutcome }
}

export interface ClientCallStateFrame {
  client_call_state: { call_id: string; state: unknown }
}

export interface ClientToolCancelledFrame {
  client_tool_cancelled: { call_id: string }
}

export interface ClientErrorFrame {
  client_error: { code: string; message: string; call_id?: string; tool?: string; rule?: string }
}

export type ClientFrame =
  | ClientToolCallFrame
  | ClientToolsResolvedFrame
  | ClientToolOutcomeFrame
  | ClientCallStateFrame
  | ClientToolCancelledFrame
  | ClientErrorFrame

// WebSocketResponse matches backend VizierResponse struct
export interface WebSocketResponse {
  timestamp: string
  content: VizierResponseContent
  attachments?: VizierAttachment[]
  // The history uid the final frame of a turn was saved as; absent on every other frame.
  history_uid?: string
  // The reply to a turn that a restart interrupted and that then carried on.
  resumed?: boolean
}

// ============================================================================
// SPEC 024 ADDITIONS
// ============================================================================

export type RecallOrigin = 'auto' | 'search'

export interface RecalledPassage {
    bundle: string
    path: string
    ordinal: number
    score: number
    title?: string
}

/** A file saved to a topic: from a tool, or attached to a request. */
export interface SessionFileRecord {
    id: string
    session_slug: string
    agent_id: string
    filename: string
    mime_type: string
    size: number
    file_id: string
    added_at: string
    /** A person's username, or the agent id. */
    sender?: string | null
    /** The request entry the file arrived with. */
    history_uid?: string | null
}

export interface SessionFileFrame {
    session_file: { file: SessionFileRecord }
}

/** Every frame the topic socket sends, once told apart by `classifyFrame`. */
export type Frame =
    | { kind: 'response'; response: WebSocketResponse }
    | { kind: 'job'; snapshot: BackgroundJobSnapshot }
    | { kind: 'history_entry'; entry: ChatMessage }
    | { kind: 'reactions'; messageUid: string; reactions: ReactionEntry[] }
    | { kind: 'reaction_error'; messageUid: string; message: string }
    | { kind: 'session_file'; file: SessionFileRecord }
    | { kind: 'client'; frame: ClientFrame }

const isObj = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v)

const CLIENT_KEYS = [
    'client_tool_call',
    'client_tools_resolved',
    'client_tool_outcome',
    'client_call_state',
    'client_tool_cancelled',
    'client_error',
] as const

/** Tells a parsed socket message apart by its top-level key; anything else is a response. */
export function classifyFrame(raw: unknown): Frame | null {
    if (!isObj(raw)) return null
    if ('background_job' in raw) return { kind: 'job', snapshot: raw.background_job as BackgroundJobSnapshot }
    if ('history_entry' in raw) return { kind: 'history_entry', entry: raw.history_entry as ChatMessage }
    if ('session_file' in raw) return { kind: 'session_file', file: (raw as unknown as SessionFileFrame).session_file.file }
    if ('reactions' in raw && isObj(raw.reactions)) {
        const r = raw.reactions as { message_uid: string; reactions: ReactionEntry[] }
        return { kind: 'reactions', messageUid: r.message_uid, reactions: r.reactions }
    }
    if ('reaction_error' in raw && isObj(raw.reaction_error)) {
        const r = raw.reaction_error as { message_uid: string; message: string }
        return { kind: 'reaction_error', messageUid: r.message_uid, message: r.message }
    }
    if (CLIENT_KEYS.some((k) => k in raw)) return { kind: 'client', frame: raw as unknown as ClientFrame }
    if ('content' in raw) return { kind: 'response', response: raw as unknown as WebSocketResponse }
    return null
}
