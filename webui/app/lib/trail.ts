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
  ExecutionReport,
  TrailEvent,
  Turn,
  VizierResponseContent,
} from '../interfaces/types'

// Same key set as the server's `ExecutionReport::looks_like`. The live stream carries no
// correlation id, so shape is the only way to tell a python report from any other tool
// response.
const REPORT_KEYS = ['ok', 'stdout', 'tool_calls', 'duration_ms'] as const

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

/** Adjacent reasoning, and adjacent narration, read as one block rather than a list. */
function push(trail: TrailEvent[], event: TrailEvent): void {
  const last = trail[trail.length - 1]
  if (
    last &&
    last.kind === event.kind &&
    (event.kind === 'thought' || event.kind === 'narration') &&
    (last.kind === 'thought' || last.kind === 'narration')
  ) {
    last.text = `${last.text}\n\n${event.text}`
    return
  }
  trail.push(event)
}

/**
 * The trail event one history entry produces, or `null` for an entry that is not part of a
 * trail — a request, an outcome, a divider, or a kind this version does not know about.
 */
function historyEvent(entry: ChatMessage): TrailEvent | null {
  const { content } = entry

  if (typeof content.AssistantMessage === 'string') {
    return content.AssistantMessage
      ? { kind: 'narration', id: entry.uid, text: content.AssistantMessage }
      : null
  }

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
  let open: Turn | null = null

  // An agent-initiated turn — a scheduled task, a dream cycle — has no request. Its trail
  // still belongs to a turn rather than being dropped.
  const newTurn = (key: string, request?: ChatMessage): Turn => {
    const turn: Turn = { key, request, trail: [], live: false }
    turns.push(turn)
    return turn
  }

  for (const entry of entries) {
    const { content } = entry

    // A divider belongs to no turn, but it does end the one that was open — so it is where
    // that turn's trail renders, live and on reload alike.
    if (content.Checkpoint !== undefined || content.Command !== undefined) {
      if (open) open.anchorUid = entry.uid
      open = null
      pendingPython = new Map()
      continue
    }

    if (content.Request !== undefined) {
      open = newTurn(entry.uid, entry)
      pendingPython = new Map()
      continue
    }

    if (content.Response !== undefined) {
      const turn = open ?? newTurn(entry.uid)
      turn.outcome = entry
      turn.anchorUid = entry.uid
      turn.key = entry.uid
      open = null
      pendingPython = new Map()
      continue
    }

    // A report pairs with its call by `call_id`, which history always carries.
    if (content.ToolResult) {
      const waiting = pendingPython.get(content.ToolResult.call_id)
      if (waiting && waiting.kind === 'python') {
        try {
          waiting.report = parseExecutionReport(JSON.parse(content.ToolResult.content))
        } catch {
          waiting.report = null
        }
        pendingPython.delete(content.ToolResult.call_id)
      }
      continue
    }

    const event = historyEvent(entry)
    if (!event) continue

    if (!open) open = newTurn(entry.uid)
    push(open.trail, event)
    if (event.kind === 'python' && content.ToolCall) {
      pendingPython.set(content.ToolCall.call_id, event)
    }
  }

  return turns
}

/**
 * One streamed frame to a trail event, or `null` for a frame that is not one.
 *
 * There is no `narration` here: intermediate assistant text is not streamed, so a turn
 * gains its narration only once reloaded from history. The asymmetry is accepted — the
 * alternative is a new WebSocket frame for a cosmetic gain.
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
  if (!report) return trail

  for (let i = trail.length - 1; i >= 0; i -= 1) {
    const event = trail[i]
    if (event.kind === 'python' && event.report === null) {
      const next = trail.slice()
      next[i] = { ...event, report }
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
    return attachReport(trail, content.tool_response.response)
  }

  const event = liveEvent(content, id)
  if (!event) return trail

  const next = trail.map((item) => ({ ...item }))
  push(next, event)
  return next
}

/** Counts by kind plus the turn's duration, bounded regardless of how long the trail is. */
export function trailSummary(trail: TrailEvent[], durationMs?: number): string {
  const counts = { thought: 0, narration: 0, tool: 0, python: 0 }
  for (const event of trail) counts[event.kind] += 1

  const parts: string[] = []
  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`
  if (counts.thought) parts.push(plural(counts.thought, 'thought', 'thoughts'))
  if (counts.narration) parts.push(plural(counts.narration, 'note', 'notes'))
  if (counts.tool) parts.push(plural(counts.tool, 'tool', 'tools'))
  if (counts.python) parts.push(plural(counts.python, 'python run', 'python runs'))

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
