// The activity trail: what an agent did on its way to an answer, normalized so the live
// WebSocket stream and stored history produce the same events and render through the same
// component.
//
// Two producers, one model. `groupHistory` reads stored history on load; `liveEvent` reads
// frames as they stream. Keeping them in one module is the point — built separately they
// would drift, and a turn would change appearance on refresh.
//
// Everything here is pure and free of JSX so it can be unit-tested without a browser.
// See `specs/010-webui-reasoning-display/contracts/trail-model.md`.

import type {
  ChatMessage,
  ClientCallStateFrame,
  ClientCallView,
  ClientToolCallFrame,
  ClientToolOutcomeFrame,
  ExecutionReport,
  TrailEvent,
  Turn,
  TurnBlock,
  VizierResponseContent,
} from '../api/frames'
import { parseDelivery } from './detached'
import { splitDelivery } from './subagents'

// Same key set as the server's `ExecutionReport::looks_like`. The live stream carries no
// correlation id, so shape is the only way to tell a python report from any other tool
// response.
const REPORT_KEYS = ['ok', 'stdout', 'tool_calls', 'duration_ms'] as const

/** The handle a detached `execute_python` returns in place of a report (spec 058), if it is one. */
export function parseDetachedHandle(value: unknown): string | null {
  if (!value || typeof value !== 'object') return null
  const v = value as Record<string, unknown>
  return v.detached === true && typeof v.id === 'string' ? v.id : null
}

export function parseExecutionReport(value: unknown): ExecutionReport | null {
  if (!value || typeof value !== 'object') return null
  return REPORT_KEYS.every((key) => key in value) ? (value as ExecutionReport) : null
}

const asString = (value: unknown): string | null =>
  typeof value === 'string' && value.length > 0 ? value : null

const asArgs = (value: unknown): Record<string, unknown> =>
  value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {}

// The tools that launch a background job, and the job id their result quotes.
// Keep `paralel_subtasks` so saved history still renders as a job launch.
// `delegate_agent` is retired (spec 053) and stays so saved history keeps its job link (#82).
const JOB_LAUNCHING_TOOLS = new Set(['paralel_subtasks', 'background_subagents', 'delegate_agent'])
const JOB_ID = /\bb-[0-9a-f]{6}\b/

/** The background job id a launching tool's result quotes, if any. */
export function launchedJobId(name: string, result: unknown): string | undefined {
  if (!JOB_LAUNCHING_TOOLS.has(name)) return undefined
  const text = typeof result === 'string' ? result : JSON.stringify(result ?? '')
  return text.match(JOB_ID)?.[0]
}

/** Adjacent reasoning reads as one block rather than a list. */
function push(trail: TrailEvent[], event: TrailEvent): void {
  const last = trail[trail.length - 1]
  if (last && last.kind === 'thought' && event.kind === 'thought') {
    last.text = `${last.text}\n\n${event.text}`
    return
  }
  trail.push(event)
}

/**
 * The trail event one history entry produces, or `null` for an entry that is not part of a
 * trail — a request, a message, an outcome, a divider, or a kind this version does not know.
 */
function historyEvent(entry: ChatMessage): TrailEvent | null {
  const { content } = entry
  const call = content.ToolCall
  if (!call) return null

  const args = asArgs(call.arguments)

  if (call.name === 'think') {
    const thought = asString(args.thought)
    return thought ? { kind: 'thought', id: entry.uid, text: thought } : null
  }

  if (call.name === 'execute_python') {
    return {
      kind: 'python',
      id: entry.uid,
      intent: asString(args.intent),
      code: asString(args.code),
      report: null,
    }
  }

  return { kind: 'tool', id: entry.uid, name: call.name, args }
}

// The server's stop notices (contracts/websocket-and-replies.md §3).
const STOP_NOTICE = /^(Stopped|Interrupted):/

/**
 * A turn-ending tool's call saves the model's text as a mid-turn message and again as the reply
 * (`chat_turn` records every step of such a turn). It was said once, so it is drawn once, as the
 * reply. Removing it can leave two trails side by side, which then read as one stretch of work.
 */
function dropEchoedMessage(turn: Turn, outcome: ChatMessage): void {
  const content = outcome.content.Response?.content
  if (typeof content !== 'object' || content === null || !('message' in content)) return
  if (!content.message.stats?.ended_by_tool) return
  const at = lastBlockOf(turn, 'message')
  const found = turn.blocks[at]
  if (!found || found.kind !== 'message' || found.text !== content.message.content) return
  turn.blocks.splice(at, 1)
  const before = turn.blocks[at - 1]
  const after = turn.blocks[at]
  if (before?.kind === 'trail' && after?.kind === 'trail') {
    before.events.push(...after.events)
    before.endedAt = after.endedAt ?? before.endedAt
    turn.blocks.splice(at, 1)
  }
}

/** The index of a turn's last block of one kind, or -1. */
export function lastBlockOf(turn: Turn, kind: TurnBlock['kind']): number {
  for (let i = turn.blocks.length - 1; i >= 0; i -= 1) {
    if (turn.blocks[i].kind === kind) return i
  }
  return -1
}

/** Every trail event of a turn, across its blocks, in order. */
export function turnEvents(turn: Turn): TrailEvent[] {
  return turn.blocks.flatMap((block) => (block.kind === 'trail' ? block.events : []))
}

/** How long one stretch of work took, when its entries carry times. */
export function blockDurationMs(block: Extract<TurnBlock, { kind: 'trail' }>): number | undefined {
  if (!block.startedAt || !block.endedAt) return undefined
  const span = Date.parse(block.endedAt) - Date.parse(block.startedAt)
  return Number.isFinite(span) && span >= 0 ? span : undefined
}

/**
 * Stored history to turns.
 *
 * Pure, and deliberately **not** sorting: the storage layer already ordered the entries by
 * `(timestamp, seq)`, and re-deriving an order here from the timestamp alone would undo it.
 * Entry order in equals event order out.
 *
 * `Checkpoint` and `Command` entries belong to no turn and are left out — they render as
 * dividers in their own right. An entry kind this version does not recognise is skipped
 * rather than throwing, so one unknown entry cannot blank a conversation.
 */
export function groupHistory(entries: ChatMessage[]): Turn[] {
  const turns: Turn[] = []
  // call_id of an unpaired `execute_python` call → the event waiting for its report.
  let pendingPython: Map<string, TrailEvent> = new Map()
  // call_id of a job-launching call → its event, waiting for the result that names the job.
  let pendingLaunch: Map<string, TrailEvent> = new Map()
  // call_id of any call event → the event, so a stop notice can mark it stopped.
  const callEvents: Map<string, TrailEvent> = new Map()
  // call_id of any call event → its trail, since a result can land after a later block began.
  const callBlocks: Map<string, Extract<TurnBlock, { kind: 'trail' }>> = new Map()
  let open: Turn | null = null

  // An agent-initiated turn — a scheduled task, a dream cycle — has no request. Its trail
  // still belongs to a turn rather than being dropped.
  const newTurn = (key: string, request?: ChatMessage): Turn => {
    const turn: Turn = { key, request, blocks: [], live: false, clientCalls: [], questions: [] }
    turns.push(turn)
    return turn
  }

  for (const entry of entries) {
    const { content } = entry

    // A divider belongs to no turn, but it does end the one that was open — so it is where
    // that turn's trail renders, live and on reload alike.
    if (content.Checkpoint !== undefined || content.Command !== undefined) {
      if (open) {
        open.anchorUid = entry.uid
        if (content.Command === 'abort' && !open.outcome) open.aborted = true
      }
      open = null
      pendingPython = new Map()
      continue
    }

    if (content.Request !== undefined) {
      // A detached script that finished while a turn ran is delivered at a round boundary
      // (spec 058) and saved as a request, but it is a step of that turn, not a new one:
      // splitting there left the work before it with no reply and no place in the
      // conversation. Only a delivery that woke an idle agent starts a turn of its own.
      const delivered = content.Request.content
      if (open && 'detached_delivery' in delivered) {
        let block = open.blocks[open.blocks.length - 1]
        if (block?.kind !== 'trail') {
          block = { kind: 'trail', events: [], startedAt: entry.timestamp }
          open.blocks.push(block)
        }
        const { scripts, workers } = splitDelivery(delivered.detached_delivery)
        for (const s of parseDelivery(scripts)) {
          push(block.events, { kind: 'delivery', id: `${entry.uid}:${s.handle}`, ...s })
        }
        for (const w of workers) {
          push(block.events, { kind: 'worker_update', id: `${entry.uid}:${w.handle}`, ...w })
        }
        if (entry.timestamp) block.endedAt = entry.timestamp
        continue
      }
      // An unattended request — a scheduled task run, a dream cycle — opens a turn without
      // becoming its request. That is what the comment above `newTurn` has always claimed,
      // and it held only because no such history had ever reached this function. A run's
      // trail is rendered under the task's own instruction, so adopting the entry here
      // would put a second, nameless copy of it on screen.
      const unattended = 'unattended' in content.Request.content
      open = newTurn(entry.uid, unattended ? undefined : entry)
      pendingPython = new Map()
      continue
    }

    if (content.Response !== undefined) {
      const turn = open ?? newTurn(entry.uid)
      dropEchoedMessage(turn, entry)
      turn.outcome = entry
      turn.anchorUid = entry.uid
      turn.key = entry.uid
      open = null
      pendingPython = new Map()
      continue
    }

    // A client tool call belongs to its turn's reply rather than its trail: it is drawn in
    // the bubble whether or not the trail is open, and also when the turn never finished.
    if (content.ClientToolCall) {
      if (!open) open = newTurn(entry.uid)
      open.clientCalls.push(clientCallView(entry))
      continue
    }

    // A question set belongs to the reply it ended the turn with: the card is drawn in the turn's
    // bubble, after the text the agent wrote beside it (spec 042).
    if (content.QuestionSet) {
      if (!open) open = newTurn(entry.uid)
      open.questions.push(entry)
      continue
    }

    // A report pairs with its call by `call_id`, which history always carries.
    if (content.ToolResult) {
      const holder = callBlocks.get(content.ToolResult.call_id)
      if (holder && entry.timestamp) holder.endedAt = entry.timestamp
      if (STOP_NOTICE.test(content.ToolResult.content)) {
        const stopped = callEvents.get(content.ToolResult.call_id)
        if (stopped && (stopped.kind === 'tool' || stopped.kind === 'python')) stopped.stopped = true
      }
      const launch = pendingLaunch.get(content.ToolResult.call_id)
      if (launch && launch.kind === 'tool') {
        launch.jobId = launchedJobId(launch.name, content.ToolResult.content)
        pendingLaunch.delete(content.ToolResult.call_id)
      }
      const waiting = pendingPython.get(content.ToolResult.call_id)
      if (waiting && waiting.kind === 'python') {
        try {
          const parsed = JSON.parse(content.ToolResult.content)
          waiting.report = parseExecutionReport(parsed)
          const handle = parseDetachedHandle(parsed)
          if (handle) waiting.detachedId = handle
        } catch {
          waiting.report = null
        }
        pendingPython.delete(content.ToolResult.call_id)
      }
      continue
    }

    // Text an agent writes between tool rounds is thinking out loud, so it stays inside the work
    // it belongs to: a step of the open trail, at the point it was written, like a thought. It is
    // not a message, so it neither splits the turn into two trails nor reads as something said to
    // anyone else (spec 053 US7, revised).
    if (typeof content.AssistantMessage === 'string') {
      // A bare "\n\n" from a model that says nothing beside a call is not a message.
      if (!content.AssistantMessage.trim()) continue
      if (!open) open = newTurn(entry.uid)
      let block = open.blocks[open.blocks.length - 1]
      if (block?.kind !== 'trail') {
        block = { kind: 'trail', events: [], startedAt: entry.timestamp }
        open.blocks.push(block)
      }
      push(block.events, { kind: 'narration', id: entry.uid, text: content.AssistantMessage })
      if (entry.timestamp) block.endedAt = entry.timestamp
      continue
    }

    const event = historyEvent(entry)
    if (!event) continue

    if (!open) open = newTurn(entry.uid)
    let block = open.blocks[open.blocks.length - 1]
    if (block?.kind !== 'trail') {
      block = { kind: 'trail', events: [], startedAt: entry.timestamp }
      open.blocks.push(block)
    }
    push(block.events, event)
    if (entry.timestamp) block.endedAt = entry.timestamp
    if (content.ToolCall) {
      callEvents.set(content.ToolCall.call_id, event)
      callBlocks.set(content.ToolCall.call_id, block)
    }
    if (event.kind === 'python' && content.ToolCall) {
      pendingPython.set(content.ToolCall.call_id, event)
    }
    if (event.kind === 'tool' && content.ToolCall && JOB_LAUNCHING_TOOLS.has(event.name)) {
      pendingLaunch.set(content.ToolCall.call_id, event)
    }
  }

  return turns
}

/** The turn still running: the last one, when nothing has closed it. */
export function openTurn(turns: Turn[]): Turn | undefined {
  const last = turns[turns.length - 1]
  return last && !last.outcome && !last.aborted && last.anchorUid === undefined ? last : undefined
}

/**
 * Folds entries into a history already held: an entry whose uid is known replaces it in
 * place, a new one goes after the last. Stored order is kept, so the same step arriving
 * twice (pushed, then read back) is shown once.
 */
export function mergeEntries(prev: ChatMessage[], incoming: ChatMessage[]): ChatMessage[] {
  const at = new Map<string, number>()
  prev.forEach((entry, i) => at.set(entry.uid, i))
  const next = prev.slice()
  for (const entry of incoming) {
    const known = at.get(entry.uid)
    if (known !== undefined) {
      next[known] = entry
    } else {
      at.set(entry.uid, next.length)
      next.push(entry)
    }
  }
  return next
}

/** A stored (or live) `ClientToolCall` entry as a turn draws it. */
export function clientCallView(entry: ChatMessage): ClientCallView {
  const call = entry.content.ClientToolCall!
  return {
    callId: entry.uid,
    name: call.name,
    category: call.category,
    tool: call.tool,
    args: asArgs(call.arguments),
    outcome: call.outcome,
    state: call.state,
    live: false,
  }
}

// The live side of client tool calls. A call arrives as a frame and becomes the same
// `ClientToolCall` entry history would hold, keyed by its call id, so `groupHistory` places
// it in its turn exactly as it will after a reload (specs/019-client-tools R11).

/** Adds a streamed call as a history entry; a second copy of the same call is ignored. */
export function appendClientCall(
  messages: ChatMessage[],
  frame: ClientToolCallFrame['client_tool_call'],
  session: ChatMessage['vizier_session']
): ChatMessage[] {
  if (messages.some((m) => m.uid === frame.call_id)) return messages
  return [
    ...messages,
    {
      uid: frame.call_id,
      vizier_session: session,
      content: {
        ClientToolCall: {
          name: frame.name,
          category: frame.category,
          tool: frame.tool,
          arguments: asArgs(frame.arguments),
          outcome: { status: 'pending' },
        },
      },
    },
  ]
}

function updateCall(
  messages: ChatMessage[],
  callId: string,
  change: (call: NonNullable<ChatMessage['content']['ClientToolCall']>) => NonNullable<ChatMessage['content']['ClientToolCall']>
): ChatMessage[] {
  const at = messages.findIndex((m) => m.uid === callId && m.content.ClientToolCall)
  if (at < 0) return messages
  const next = messages.slice()
  const message = next[at]
  next[at] = { ...message, content: { ...message.content, ClientToolCall: change(message.content.ClientToolCall!) } }
  return next
}

/** Records how a call ended. */
export function applyClientOutcome(
  messages: ChatMessage[],
  frame: ClientToolOutcomeFrame['client_tool_outcome']
): ChatMessage[] {
  return updateCall(messages, frame.call_id, (call) => ({ ...call, outcome: frame.outcome }))
}

/** Records a call's newly saved state. */
export function applyClientState(
  messages: ChatMessage[],
  frame: ClientCallStateFrame['client_call_state']
): ChatMessage[] {
  return updateCall(messages, frame.call_id, (call) => ({ ...call, state: frame.state }))
}

/**
 * One streamed frame to a trail event, or `null` for a frame that is not one. Text the agent
 * writes between rounds is not one of them either: it reaches a running turn as a `history_entry`
 * frame, like every saved step, and `groupHistory` places it in the trail.
 */
export function liveEvent(content: VizierResponseContent, id: string): TrailEvent | null {
  if (typeof content !== 'object' || content === null) return null

  if ('thinking' in content) {
    return content.thinking ? { kind: 'thought', id, text: content.thinking } : null
  }

  if ('tool_choice' in content) {
    const { name, args } = content.tool_choice
    const argsObj = asArgs(args)

    if (name === 'think') {
      const thought = asString(argsObj.thought)
      return thought ? { kind: 'thought', id, text: thought } : null
    }

    if (name === 'execute_python') {
      return {
        kind: 'python',
        id,
        intent: asString(argsObj.intent),
        code: asString(argsObj.code),
        report: null,
      }
    }

    return { kind: 'tool', id, name, args: argsObj }
  }

  return null
}

/**
 * The live counterpart of history's `call_id` pairing.
 *
 * `ToolChoice` carries no correlation id, so a report attaches to the most recent python
 * event still waiting for one. Returns the trail unchanged when the frame is not a report
 * or there is nothing waiting for it.
 */
export function attachReport(trail: TrailEvent[], response: unknown): TrailEvent[] {
  const report = parseExecutionReport(response)
  const handle = report ? null : parseDetachedHandle(response)
  if (!report && !handle) return trail

  for (let i = trail.length - 1; i >= 0; i -= 1) {
    const event = trail[i]
    if (event.kind === 'python' && event.report === null && !event.detachedId) {
      const next = trail.slice()
      next[i] = report ? { ...event, report } : { ...event, detachedId: handle ?? undefined }
      return next
    }
  }

  return trail
}

/** Appends a streamed frame to a live trail, pairing a python report with its run. */
export function appendLiveEvent(
  trail: TrailEvent[],
  content: VizierResponseContent,
  id: string
): TrailEvent[] {
  if (typeof content === 'object' && content !== null && 'tool_response' in content) {
    const response = content.tool_response.response
    // Like a python report, a launch result pairs with the most recent launching call that
    // has not been given one.
    for (let i = trail.length - 1; i >= 0; i -= 1) {
      const event = trail[i]
      if (event.kind === 'tool' && JOB_LAUNCHING_TOOLS.has(event.name) && !event.jobId) {
        const jobId = launchedJobId(event.name, response)
        if (!jobId) break
        const next = trail.slice()
        next[i] = { ...event, jobId }
        return next
      }
    }
    return attachReport(trail, response)
  }

  const event = liveEvent(content, id)
  if (!event) return trail

  const next = trail.map((item) => ({ ...item }))
  push(next, event)
  return next
}

/** Counts by kind plus the turn's duration, bounded regardless of how long the trail is. */
export function trailSummary(trail: TrailEvent[], durationMs?: number): string {
  // A narration is the agent's prose between rounds, counted with its reasoning: the head says
  // what the work was, not that it spoke.
  const counts = { thought: 0, tool: 0, python: 0, delivery: 0, worker_update: 0 }
  for (const event of trail) counts[event.kind === 'narration' ? 'thought' : event.kind] += 1

  const parts: string[] = []
  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`
  if (counts.thought) parts.push(plural(counts.thought, 'thought', 'thoughts'))
  if (counts.tool) parts.push(plural(counts.tool, 'tool', 'tools'))
  if (counts.python) parts.push(plural(counts.python, 'python run', 'python runs'))
  if (counts.delivery) parts.push(plural(counts.delivery, 'background result', 'background results'))
  if (counts.worker_update) parts.push(plural(counts.worker_update, 'subagent update', 'subagent updates'))

  const label = parts.length > 0 ? parts.join(', ') : 'no activity'
  return durationMs === undefined
    ? `Reasoning · ${label}`
    : `Reasoning · ${label} · ${(durationMs / 1000).toFixed(1)}s`
}

/** The turn duration a closing `Response` reports, in milliseconds, when it reports one. */
export function outcomeDurationMs(outcome?: ChatMessage): number | undefined {
  const content = outcome?.content.Response?.content
  if (typeof content !== 'object' || content === null) return undefined

  const stats =
    'message' in content
      ? content.message.stats
      : 'audio_reply' in content
        ? (content.audio_reply[2] ?? undefined)
        : undefined
  if (!stats) return undefined

  return stats.duration.secs * 1000 + Math.round(stats.duration.nanos / 1_000_000)
}

/**
 * How many dots of one state the phone's summary line shows before it counts the rest. A couple of
 * failures and the newest of the rest fit beside the title in a phone drawer; the row clips the
 * strip rather than grow, so this is a cap on noise, not on width.
 */
export const MINI_DOTS_PER_STATE = 3

export type MiniDot = 'done' | 'running' | 'failed'

/**
 * The phone trail's dots: at most `per` of each state, taken from the newest calls, and how many
 * calls went unshown. Proportional dots would stretch the chip with the work done — a hundred
 * calls is a hundred dots — so the strip is bounded and the remainder is counted instead.
 */
export function miniDots(
    dots: MiniDot[],
    per = MINI_DOTS_PER_STATE
): { dots: MiniDot[]; more: number } {
    const kept: MiniDot[] = []
    const seen: Record<MiniDot, number> = { done: 0, running: 0, failed: 0 }
    // Newest first, so a failure or a running call at the end of the turn survives the cap.
    for (let i = dots.length - 1; i >= 0; i--) {
        const dot = dots[i]
        if (seen[dot] < per) {
            seen[dot] += 1
            kept.push(dot)
        }
    }
    // Restore the order they happened in.
    kept.reverse()
    return { dots: kept, more: dots.length - kept.length }
}
