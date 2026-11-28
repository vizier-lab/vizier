import type { BadgeTone, StatusState } from '@vizier/design'
import type { KindField, Trigger, TriggerEvent, TriggerInput, TriggerKind, TriggerRun } from '../api/endpoints/triggers'
import { relativeTime } from './format'

/** Whether a monitor's checks have stopped after failing too often. */
export const stopped = (t: Pick<Trigger, 'monitor'>) => !!t.monitor?.failed_at

/** Taking events now: switched on, approved and, for a monitor, still checking. */
export const isActive = (t: Trigger) => t.is_active && !t.awaiting_approval && !stopped(t)

export const TRIGGER_FILTERS = ['all', 'active', 'needs_you'] as const
export type TriggerFilter = (typeof TRIGGER_FILTERS)[number]

export const FILTER_LABELS: Record<TriggerFilter, string> = { all: 'All', active: 'Active', needs_you: 'Needs you' }

export function matches(t: Trigger, filter: string, q: string): boolean {
    if (filter === 'active' && !isActive(t)) return false
    if (filter === 'needs_you' && !t.needs_person) return false
    const needle = q.trim().toLowerCase()
    return !needle || [t.title, t.slug, t.instruction].some((s) => s.toLowerCase().includes(needle))
}

export function counts(triggers: Trigger[]): Record<TriggerFilter, number> {
    return {
        all: triggers.length,
        active: triggers.filter(isActive).length,
        needs_you: triggers.filter((t) => t.needs_person).length,
    }
}

export const SORTS = [
    { value: 'last_fired', label: 'Last fired' },
    { value: 'name', label: 'Name' },
]

export function sorted(triggers: Trigger[], sort: string): Trigger[] {
    const at = (t: Trigger) => (t.last_fired_at ? Date.parse(t.last_fired_at) : 0)
    return [...triggers].sort((a, b) => (sort === 'name' ? a.title.localeCompare(b.title) : at(b) - at(a) || a.title.localeCompare(b.title)))
}

export const RUN_STATES: Record<TriggerRun['state'], Extract<StatusState, 'answered' | 'running' | 'no-response' | 'interrupted'>> = {
    running: 'running',
    answered: 'answered',
    no_response: 'no-response',
    interrupted: 'interrupted',
}

export function runLabel(run: TriggerRun): string {
    if (run.state === 'answered') return run.quiet_note ? 'Nothing to report' : 'Answered'
    return { running: 'Running', no_response: 'No response', interrupted: 'Interrupted' }[run.state]
}

/** The list's "Latest" cell: what most needs saying about a trigger now. */
export function latest(t: Trigger, now: number = Date.now()): { state: StatusState; label: string; detail?: string } {
    if (t.awaiting_approval) return { state: 'retrying', label: 'Waiting for approval' }
    if (stopped(t)) return { state: 'failed', label: 'Stopped', detail: failedChecks(t.monitor!.failures) }
    if (t.needs_attention) return { state: 'retrying', label: 'Needs attention', detail: t.needs_attention }
    const run = t.latest_run
    if (run?.state === 'running') return { state: 'running', label: 'Running' }
    // A failing check is newer news than the last run.
    if (t.monitor?.last_error) return { state: 'retrying', label: 'Check failing', detail: failedChecks(t.monitor.failures) }
    if (run) return { state: RUN_STATES[run.state], label: runLabel(run), detail: relativeTime(run.started_at, now) }
    if (t.monitor?.last_check_at) return { state: 'idle', label: 'Quiet', detail: `checked ${relativeTime(t.monitor.last_check_at, now)}` }
    return { state: 'not-run', label: t.is_active ? 'Not run yet' : 'Paused' }
}

const failedChecks = (n: number) => `${n} failed check${n === 1 ? '' : 's'}`

/** An event's outcome as its badge. */
export function outcome(e: Pick<TriggerEvent, 'status' | 'count'>): { label: string; tone: BadgeTone } {
    const base: Record<TriggerEvent['status'], { label: string; tone: BadgeTone }> = {
        triage_pending: { label: 'Waiting for triage', tone: 'retrying' },
        waiting: { label: 'Waiting', tone: 'retrying' },
        handled: { label: 'Handled', tone: 'live' },
        unimportant: { label: 'Not important', tone: 'neutral' },
        ignored: { label: 'Ignored', tone: 'neutral' },
        dropped: { label: 'Dropped', tone: 'retrying' },
        refused: { label: 'Refused', tone: 'retrying' },
        failed: { label: 'Failed', tone: 'failed' },
    }
    const { label, tone } = base[e.status]
    return { label: e.count > 1 ? `${label} ×${e.count}` : label, tone }
}

export function eventReason(e: TriggerEvent): string {
    if (e.untriaged) return 'Triage failed, so it counted as important'
    if (e.reason) return e.reason
    return { waiting: 'Waits for a run', triage_pending: 'Waits for triage', handled: 'Given to a run' }[e.status as string] ?? '–'
}

/** One line of a payload, for its row; the whole of it is in the run. */
export function payloadLine(payload: unknown): string {
    if (payload === null || payload === undefined) return 'payload not kept'
    const text = typeof payload === 'string' ? payload : JSON.stringify(payload)
    return text.length > 160 ? `${text.slice(0, 160)}…` : text
}

export function intervalText(seconds: number): string {
    if (seconds % 3600 === 0) return `every ${seconds / 3600} h`
    if (seconds % 60 === 0 && seconds >= 120) return `every ${seconds / 60} min`
    return `every ${seconds} s`
}

// --- tool access -------------------------------------------------------------------------------

export const MEMORY_READ = 'memory_read'
/** The categories that reach a person on their own, which the run limit cannot bound. */
export const MESSAGING = ['discord', 'telegram', 'webui', 'sessions']

/** The three forms a saved scope takes: every tool, only some, or all but some. */
export type ScopeMode = 'every' | 'only' | 'except'

export function scopeMode(scope: string[] | null | undefined): ScopeMode {
    if (scope === null || scope === undefined) return 'every'
    return scope.length > 0 && scope.every((e) => e.startsWith('!')) ? 'except' : 'only'
}

/** The categories a scope names, without the exclusion mark. */
export const scopeNames = (scope: string[] | null) => (scope ?? []).map((e) => e.replace(/^!/, ''))

/** Where a person starts after choosing another mode; nothing is converted from the old one. */
export function scopeFor(mode: ScopeMode): string[] | null {
    if (mode === 'every') return null
    return mode === 'only' ? [MEMORY_READ] : MESSAGING.map((c) => `!${c}`)
}

export function toggleScope(scope: string[], name: string, on: boolean, mode: ScopeMode): string[] {
    const entry = mode === 'except' ? `!${name}` : name
    const rest = scope.filter((e) => e !== entry)
    return on ? [...rest, entry] : rest
}

/** The same rule as the server's: whether runs under `scope` may message or wake. */
export function allowsMessaging(scope: string[] | null): boolean {
    if (scope === null) return true
    if (scope.length === 0) return false
    const excluded = (c: string) => scope.includes(`!${c}`)
    const listed = scope.some((e) => !e.startsWith('!'))
    return MESSAGING.some((c) => !excluded(c) && (!listed || scope.includes(c)))
}

// --- the settings form -------------------------------------------------------------------------

/** What a field shows: lists are edited one per line, everything else as itself. */
export type FieldValues = Record<string, string | boolean | string[]>

/** The form's starting values. Secrets stay out: a blank one keeps what is saved. */
export function fieldValues(fields: KindField[], settings: Record<string, unknown> = {}): FieldValues {
    const values: FieldValues = {}
    for (const f of fields) {
        if (f.generated) continue
        const saved = f.type === 'secret' ? undefined : settings[f.key]
        const v = saved ?? (f.default as unknown)
        if (f.type === 'bool') values[f.key] = Boolean(v)
        else if (f.type === 'tool_categories') values[f.key] = Array.isArray(v) ? v.map(String) : []
        else if (f.type === 'string_list') values[f.key] = Array.isArray(v) ? v.join('\n') : String(v ?? '')
        else values[f.key] = v === undefined || v === null ? '' : String(v)
    }
    return values
}

/**
 * The settings a save sends: what the form holds over what is saved, so a setting the form does
 * not show survives. A field left blank is left out, which also keeps a saved secret.
 */
export function settingsFrom(fields: KindField[], values: FieldValues, saved: Record<string, unknown> = {}): Record<string, unknown> {
    const out: Record<string, unknown> = { ...saved }
    for (const f of fields) {
        if (f.type === 'secret' || f.generated) delete out[f.key]
        if (f.generated) continue
        const v = values[f.key]
        if (f.type === 'bool') out[f.key] = Boolean(v)
        else if (f.type === 'tool_categories') out[f.key] = v as string[]
        else if (f.type === 'string_list') {
            const lines = String(v ?? '').split('\n').map((s) => s.trim()).filter(Boolean)
            if (lines.length) out[f.key] = lines
            else delete out[f.key]
        } else {
            const text = String(v ?? '')
            // An untouched default stays unsaved: the server applies its own.
            if (text.trim() === '' || (!(f.key in saved) && text === String(f.default ?? ''))) delete out[f.key]
            else if (f.type === 'number') out[f.key] = Number(text)
            else out[f.key] = f.type === 'code' ? text : text.trim()
        }
    }
    return out
}

export const missingFields = (fields: KindField[], values: FieldValues) =>
    fields.filter((f) => f.required && !f.generated && String(values[f.key] ?? '').trim() === '')

export const kindOf = (kinds: TriggerKind[] | null, id: string) => kinds?.find((k) => k.id === id)

/** A whole number from a field, or `undefined` when it holds something else. */
export function wholeNumber(text: string): number | undefined {
    const n = Number(text.trim())
    return text.trim() !== '' && Number.isInteger(n) && n >= 0 ? n : undefined
}

export type { TriggerInput }

// --- a secret, shown once ----------------------------------------------------------------------

/**
 * A new secret on its way to the page that shows it, held in memory only: never in the address,
 * the history state or storage, so a reload or Back cannot bring it back.
 */
let pending: { slug: string; agentId: string; secret: string } | null = null

export function holdSecret(agentId: string, slug: string, secret: string) {
    pending = { agentId, slug, secret }
}

/** The held secret, once. */
export function takeSecret(agentId: string, slug: string): string | null {
    if (!pending || pending.agentId !== agentId || pending.slug !== slug) return null
    const { secret } = pending
    pending = null
    return secret
}
