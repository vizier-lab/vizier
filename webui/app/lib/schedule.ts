import { Cron } from 'croner'
import type { Task, TaskRun } from '../api/endpoints/tasks'
import { describeCron } from './cron'
import { isCron } from './tasks'

/** What the Schedule page's filter tabs can say. `active` is the default. */
export type ScheduleFilter = 'active' | 'last_run_failed' | 'all'
export const SCHEDULE_FILTERS: ScheduleFilter[] = ['active', 'last_run_failed', 'all']
export const DEFAULT_SCHEDULE_FILTER: ScheduleFilter = 'active'

/** The same rules the server applies to the paged table, so both views narrow alike. */
export function matchesFilter(task: Task, filter: string): boolean {
    if (filter === 'active') return task.is_active
    if (filter === 'last_run_failed') return task.last_run?.state === 'no_response' || task.last_run?.state === 'interrupted'
    return true
}

export function matchesQuery(task: Task, q: string): boolean {
    const needle = q.trim().toLowerCase()
    return !needle || task.title.toLowerCase().includes(needle) || task.slug.toLowerCase().includes(needle)
}

export function filterTasks(tasks: Task[], filter: string, q: string): Task[] {
    return tasks.filter((t) => matchesFilter(t, filter) && matchesQuery(t, q))
}

export function countTasks(tasks: Task[]): Record<ScheduleFilter, number> {
    return {
        active: tasks.filter((t) => matchesFilter(t, 'active')).length,
        last_run_failed: tasks.filter((t) => matchesFilter(t, 'last_run_failed')).length,
        all: tasks.length,
    }
}

// ---- days -------------------------------------------------------------------

const pad = (n: number) => String(n).padStart(2, '0')

/** "2026-10-30", the local calendar day of a moment. */
export const dayKey = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`

/** Local midnight of a "2026-10-30", or null when it isn't a real date. */
export function parseDayKey(key: string | null | undefined): Date | null {
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(key ?? '')
    if (!m) return null
    const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]))
    return dayKey(d) === key ? d : null
}

export const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate())

/** Whole days, by calendar, so a change of clocks never skips or repeats a day. */
export const addDays = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n)

export const addMonths = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth() + n, 1)

/** The weeks (Monday first) that cover the month `anchor` is in, padded with its neighbours' days. */
export function monthWeeks(anchor: Date): Date[][] {
    const first = new Date(anchor.getFullYear(), anchor.getMonth(), 1)
    const lead = (first.getDay() + 6) % 7
    const last = new Date(anchor.getFullYear(), anchor.getMonth() + 1, 0)
    const weeks = Math.ceil((lead + last.getDate()) / 7)
    return Array.from({ length: weeks }, (_, w) => Array.from({ length: 7 }, (_, d) => addDays(first, w * 7 + d - lead)))
}

export const clock = (d: Date) => `${pad(d.getHours())}:${pad(d.getMinutes())}`

// ---- moments ----------------------------------------------------------------

export type ChipState = 'answered' | 'running' | 'no-response' | 'interrupted' | 'upcoming' | 'past' | 'paused'

/** One moment a task is scheduled for. */
export type Moment = { task: Task; at: Date; state: ChipState }

/** Most moments listed for one task on one day; a task busier than this counts as "200+". */
export const MOMENT_CAP = 200

/** A run lands on the minute its schedule asked for; this much slack covers the seconds. */
const MATCH_MS = 90_000

const crons = new Map<string, Cron | null>()

/** The server evaluates cron expressions in UTC, so the calendar does too. */
function cronOf(expression: string): Cron | null {
    if (!crons.has(expression)) {
        try {
            crons.set(expression, new Cron(expression, { timezone: 'UTC' }))
        } catch {
            crons.set(expression, null)
        }
    }
    return crons.get(expression) ?? null
}

/** Every moment the schedule names in [from, to), up to the cap. */
export function scheduledAt(task: Task, from: Date, to: Date, cap = MOMENT_CAP): { at: Date[]; capped: boolean } {
    if (!isCron(task.schedule)) {
        const at = new Date(task.schedule.OneTimeTask)
        return { at: at >= from && at < to ? [at] : [], capped: false }
    }
    const cron = cronOf(task.schedule.CronTask)
    if (!cron) return { at: [], capped: false }
    const at: Date[] = []
    let next = cron.nextRun(new Date(from.getTime() - 1))
    while (next && next < to) {
        if (at.length >= cap) return { at, capped: true }
        at.push(next)
        next = cron.nextRun(next)
    }
    return { at, capped: false }
}

const RUN_CHIP: Record<TaskRun['state'], ChipState> = {
    running: 'running',
    answered: 'answered',
    no_response: 'no-response',
    interrupted: 'interrupted',
}

/** How a moment reads: the run that happened there, what is ahead, a paused task, or an unrecorded past. */
export function momentState(task: Task, at: Date, now: Date): ChipState {
    const run = task.last_run
    if (run && Math.abs(Date.parse(run.ran_at) - at.getTime()) <= MATCH_MS) return RUN_CHIP[run.state]
    if (!task.is_active) return at > now ? 'paused' : 'past'
    return at > now ? 'upcoming' : 'past'
}

export type Moments = { moments: Moment[]; capped: boolean }

export function momentsOf(task: Task, from: Date, to: Date, now: Date): Moments {
    const { at, capped } = scheduledAt(task, from, to)
    return { moments: at.map((a) => ({ task, at: a, state: momentState(task, a, now) })), capped }
}

// ---- a day's chips ----------------------------------------------------------

export type DayChip =
    | { kind: 'single'; key: string; task: Task; at: Date; state: ChipState }
    /** A task that runs several times that day: one chip carrying the count. */
    | { kind: 'repeat'; key: string; task: Task; count: number; capped: boolean; moments: Moment[]; state: ChipState }

/** Runs a task may have in a day and still be told apart one by one; busier ones live in the day's repeat card. */
export const BLOCKS_PER_DAY = 6

/**
 * What a day shows for each task. The past is only what is on record, the task's latest run; the
 * future is the schedule. A task with several moments that day is one repeat chip.
 */
export function dayChips(tasks: Task[], day: Date, now: Date): DayChip[] {
    const from = startOfDay(day)
    const to = addDays(from, 1)
    const out: DayChip[] = []
    for (const task of tasks) {
        const { moments, capped } = momentsOf(task, from, to, now)
        if (moments.length >= 2) {
            out.push({ kind: 'repeat', key: `${task.slug}@${dayKey(from)}`, task, count: moments.length, capped, moments, state: 'upcoming' })
            continue
        }
        const only = moments[0]
        if (only && only.state !== 'past') out.push({ kind: 'single', key: `${task.slug}@${only.at.toISOString()}`, task, at: only.at, state: only.state })
    }
    return out.sort(compareChips)
}

const compareChips = (a: DayChip, b: DayChip) =>
    Number(b.kind === 'repeat') - Number(a.kind === 'repeat') ||
    (a.kind === 'single' && b.kind === 'single' ? a.at.getTime() - b.at.getTime() : 0) ||
    a.task.title.localeCompare(b.task.title)

export type MonthDay = { date: Date; key: string; outside: boolean; chips: DayChip[] }

export function monthGrid(tasks: Task[], anchor: Date, now: Date): MonthDay[][] {
    return monthWeeks(anchor).map((week) =>
        week.map((date) => ({ date, key: dayKey(date), outside: date.getMonth() !== anchor.getMonth(), chips: dayChips(tasks, date, now) }))
    )
}

/** The count a repeat chip prints. */
export const repeatText = (c: Extract<DayChip, { kind: 'repeat' }>) => `${c.count}${c.capped ? '+' : ''}×`

// ---- the day view -----------------------------------------------------------

export type RepeatRow = {
    task: Task
    cadence: string
    moments: Moment[]
    capped: boolean
    /** More than a few: shown as a count and a bar, not as a block each. */
    dense: boolean
    next: Date | null
    ahead: number
}

export type DayView = { blocks: Moment[]; repeats: RepeatRow[] }

/** A day's timeline blocks and its "repeats today" card. */
export function dayView(tasks: Task[], day: Date, now: Date): DayView {
    const blocks: Moment[] = []
    const repeats: RepeatRow[] = []
    const from = startOfDay(day)
    const to = addDays(from, 1)
    for (const task of tasks) {
        const { moments, capped } = momentsOf(task, from, to, now)
        const dense = moments.length > BLOCKS_PER_DAY
        if (moments.length >= 2) {
            const ahead = moments.filter((m) => m.at > now)
            repeats.push({
                task,
                cadence: isCron(task.schedule) ? (describeCron(task.schedule.CronTask) ?? task.schedule.CronTask) : 'Once',
                moments,
                capped,
                dense,
                next: ahead[0]?.at ?? null,
                ahead: ahead.length,
            })
        }
        if (!dense) blocks.push(...moments.filter((m) => m.state !== 'past'))
    }
    repeats.sort((a, b) => (a.next?.getTime() ?? Infinity) - (b.next?.getTime() ?? Infinity) || a.task.title.localeCompare(b.task.title))
    return { blocks, repeats }
}
