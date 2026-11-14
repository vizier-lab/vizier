import type { SubagentSnapshot, SubagentState, SubagentStopReason } from '../api/frames'

/** The Subagents panel's tabs (spec 059, frame "059 · A"). `all` is no filter. */
export type WorkerTab = 'all' | 'running' | 'asking' | 'done' | 'stopped'

/** Which tab a worker falls under: done ended on its own, stopped was cut short by a stop. */
export function workerTab(state: SubagentState): Exclude<WorkerTab, 'all'> {
    if (state === 'running' || state === 'asking' || state === 'stopped') return state
    return 'done'
}

/** The counts the tabs show. */
export function workerCounts(workers: SubagentSnapshot[]) {
    const counts = { all: workers.length, running: 0, asking: 0, done: 0, stopped: 0 }
    for (const w of workers) counts[workerTab(w.state)] += 1
    return counts
}

/** What the strip item says: the unfinished workers, with an amber dot while one is asking. */
export function workerStrip(workers: SubagentSnapshot[]): { count: number; dot?: 'live' | 'warn' } {
    const counts = workerCounts(workers)
    const count = counts.running + counts.asking
    return { count, dot: counts.asking > 0 ? 'warn' : counts.running > 0 ? 'live' : undefined }
}

/** The panel's meta line: "1 running · 1 asking", or the total when nothing is at work. */
export function workerMeta(workers: SubagentSnapshot[]): string {
    const counts = workerCounts(workers)
    const parts = [
        counts.running ? `${counts.running} running` : '',
        counts.asking ? `${counts.asking} asking` : '',
    ].filter(Boolean)
    return parts.length ? parts.join(' · ') : `${counts.all}`
}

/** One spawn's workers, in their positions. */
export type WorkerGroup = {
    id: string
    /** When the first of them was hired. */
    startedAt: string
    /** How many the spawn hired, shown or not. */
    size: number
    /** The detached script that hired them, if one did. */
    script: string | null
    workers: SubagentSnapshot[]
}

const matches = (query: string, w: SubagentSnapshot) => {
    const q = query.trim().toLowerCase()
    if (!q) return true
    return [w.task, w.id, w.question, w.result].some((p) => p?.toLowerCase().includes(q))
}

/**
 * The workers the panel shows, grouped by the spawn that hired them, newest spawn first and each
 * group in position order. The query narrows by task, handle, question or result; the tab by
 * state. A group with nothing left to show is left out.
 */
export function groupWorkers(workers: SubagentSnapshot[], query: string, tab: WorkerTab): WorkerGroup[] {
    const groups = new Map<string, WorkerGroup>()
    for (const w of workers) {
        let g = groups.get(w.group)
        if (!g) {
            g = { id: w.group, startedAt: w.started_at, size: w.group_size, script: w.parent_detached_id, workers: [] }
            groups.set(w.group, g)
        }
        if (w.started_at < g.startedAt) g.startedAt = w.started_at
        if (tab !== 'all' && workerTab(w.state) !== tab) continue
        if (!matches(query, w)) continue
        g.workers.push(w)
    }
    return [...groups.values()]
        .filter((g) => g.workers.length > 0)
        .map((g) => ({ ...g, workers: [...g.workers].sort((a, b) => a.position - b.position) }))
        .sort((a, b) => b.startedAt.localeCompare(a.startedAt) || b.id.localeCompare(a.id))
}

/** "m:ss" from hire to the end, or to `now` while unfinished. */
export function workerElapsed(w: Pick<SubagentSnapshot, 'started_at' | 'ended_at'>, now: number): string {
    const end = w.ended_at ? Date.parse(w.ended_at) : now
    const s = Math.max(0, Math.round((end - Date.parse(w.started_at)) / 1000))
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

/** A group's heading: "3 workers · hired 1:12 ago", led by "from d_…" when a script hired them. */
export function groupHeading(g: WorkerGroup, now: number): string {
    const parts = [
        g.script ? `from ${g.script}` : null,
        `${g.size} worker${g.size === 1 ? '' : 's'}`,
        `hired ${workerElapsed({ started_at: g.startedAt, ended_at: null }, now)} ago`,
    ]
    return parts.filter(Boolean).join(' · ')
}

/** Why a stopped worker stopped, for its line. */
export function stopReasonText(reason: SubagentStopReason | null, agentName: string): string | undefined {
    if (!reason) return undefined
    if (reason === 'by_agent') return `by ${agentName}`
    return reason === 'conversation_deleted' ? 'the conversation was deleted' : 'the agent was deleted'
}

/** One worker outcome a `[Subagent update]` delivery carries (`render_update`'s format). */
export type WorkerUpdate = {
    handle: string
    position: number
    size: number
    title: string
    state: string
    /** The question, result or error lines, without the instruction addressed to the agent. */
    body: string
}

const UPDATE_HEAD = /^\[Subagent update\] (s_[0-9a-z]+) — task (\d+) of (\d+) "(.*)"$/

/**
 * A delivery's parts (spec 059): the detached scripts' section, for `parseDelivery`, and the
 * worker updates. One delivery may carry both, scripts first.
 */
export function splitDelivery(text: string): { scripts: string; workers: WorkerUpdate[] } {
    const scripts: string[] = []
    const workers: WorkerUpdate[] = []
    let current: WorkerUpdate | null = null
    const body: string[] = []
    const close = () => {
        if (current) current.body = body.join('\n').trim()
        body.length = 0
    }
    for (const line of text.split('\n')) {
        const head = UPDATE_HEAD.exec(line)
        if (head) {
            close()
            current = { handle: head[1], position: Number(head[2]), size: Number(head[3]), title: head[4], state: '', body: '' }
            workers.push(current)
            continue
        }
        if (!current) {
            scripts.push(line)
            continue
        }
        const state = /^state: (.+)$/.exec(line)
        if (state && !current.state) {
            current.state = state[1]
            continue
        }
        if (/^reply with reply_subagent\(/.test(line) || /^\(read the whole result with get_subagent\(/.test(line)) continue
        body.push(line)
    }
    close()
    return { scripts: scripts.join('\n').trim(), workers }
}
