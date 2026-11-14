import type {
  DetachedScriptDetail,
  DetachedScriptSnapshot,
  ExecutionReport,
  ToolInvocationRecord,
} from '../api/frames'

/** "0:41", "12:05": from launch to the end, or to `now` while it runs. */
export function scriptElapsed(script: DetachedScriptSnapshot, now: number): string {
  const end = script.finished_at ? Date.parse(script.finished_at) : now
  const s = Math.max(0, Math.round((end - Date.parse(script.created_at)) / 1000))
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

const STOPPED: Record<string, string> = {
  timed_out: 'Ran out of time',
  interrupted: 'Interrupted by a restart',
  cancelled: 'Cancelled by the agent',
}

/**
 * A script's detail as the report the activity trail draws (spec 058): its calls so far while it
 * runs, its own report once it ended, and a limit block when it was stopped with a reason.
 */
export function detachedReport(detail: DetachedScriptDetail): ExecutionReport {
  const calls = (detail.tool_calls ?? []).map(
    (c): ToolInvocationRecord => ({
      seq: c.seq,
      name: c.name,
      arguments: c.arguments && typeof c.arguments === 'object' ? (c.arguments as Record<string, unknown>) : undefined,
      ok: c.ok,
      error: c.error ?? null,
      duration_ms: c.duration_ms,
    })
  )
  const report = detail.report
  if (report && typeof report === 'object') {
    return { ...(report as unknown as ExecutionReport), tool_calls: calls }
  }
  if (typeof report === 'string' || detail.state === 'cancelled') {
    return {
      ok: false,
      result: null,
      stdout: '',
      error: { kind: 'limit', message: STOPPED[detail.state] ?? 'Stopped', limit: typeof report === 'string' ? report : null },
      tool_calls: calls,
      duration_ms: 0,
    }
  }
  return { ok: true, result: null, stdout: '', error: null, tool_calls: calls, duration_ms: 0 }
}

/** The handles a delivery names, one per script it carries, in order. */
export function deliveredHandles(text: string): string[] {
  return [...text.matchAll(/^(d_[0-9a-z]{8}) {2}/gm)].map((m) => m[1])
}

/** A delivery without its heading line, for showing in place. */
export function deliveryBody(text: string): string {
  return text.replace(/^\[Background work finished\]\n?/, '')
}

/** One script a delivery carries: its first line, and the lines under it. */
export type DeliveredScript = { handle: string; state: string; took: string; intent: string; body: string }

const SCRIPT_LINE = /^(d_[0-9a-z]{8}) {2}(.+?) in (.+?) {2}— (.*)$/

/** The scripts a delivery carries, in order (`render_delivery`'s format). */
export function parseDelivery(text: string): DeliveredScript[] {
  const scripts: DeliveredScript[] = []
  const body: string[] = []
  const close = () => {
    const last = scripts[scripts.length - 1]
    if (last) last.body = body.join('\n').trim()
    body.length = 0
  }
  for (const line of deliveryBody(text).split('\n')) {
    const m = line.match(SCRIPT_LINE)
    if (m) {
      close()
      scripts.push({ handle: m[1], state: m[2], took: m[3], intent: m[4], body: '' })
    } else {
      body.push(line)
    }
  }
  close()
  return scripts
}
