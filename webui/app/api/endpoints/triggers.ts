import { api } from '../client'
import type { components } from '../schema'
import type { ChatMessage } from '../frames'

type Schemas = components['schemas']

export type TriageConfig = { criteria: string; provider?: string | null; model?: string | null; calls_per_hour: number }
export type OverlapPolicy = Schemas['OverlapPolicy']
export type EventStatus = Schemas['EventStatus']
export type TriggerRunState = Schemas['TriggerRunState']
export type KindField = Schemas['KindField']
export type Requester = { user: string } | { agent: string }

export type TriggerRun = {
    id: number
    started_at: string
    finished_at?: string | null
    state: TriggerRunState
    delivered_at?: string | null
    quiet_note?: string | null
}

export type MonitorHealth = {
    last_check_at?: string | null
    next_check_at?: string | null
    last_error?: string | null
    failures: number
    failed_at?: string | null
}

export type Trigger = {
    id: string
    agent_id: string
    slug: string
    kind: string
    title: string
    instruction: string
    settings: Record<string, unknown>
    requester: Requester
    delivers_to?: string | null
    is_active: boolean
    awaiting_approval?: boolean
    needs_attention?: string | null
    /** `null` is every tool. */
    tool_scope: string[] | null
    overlap: OverlapPolicy
    hourly_limit: number | null
    cooldown_secs: number
    triage: TriageConfig | null
    destination: string | null
    summary_hours: number | null
    created_at: string
    last_fired_at?: string | null
    address_path?: string | null
    allows_messaging: boolean
    monitor?: MonitorHealth | null
    latest_run?: TriggerRun | null
    runs_last_hour: number
    needs_person: boolean
}

/** What a person sets. On an update, a field left out keeps its value; `null` clears it. */
export type TriggerInput = Partial<
    Pick<
        Trigger,
        'slug' | 'kind' | 'title' | 'instruction' | 'settings' | 'is_active' | 'tool_scope' | 'overlap' | 'hourly_limit' | 'cooldown_secs' | 'triage' | 'destination' | 'summary_hours'
    >
>

export type TriggerEvent = {
    id: number
    received_at: string
    payload?: unknown
    status: EventStatus
    reason?: string | null
    untriaged: boolean
    /** More than one on a record that collapses a storm of drops. */
    count: number
    run_id?: number | null
}

export type TriggerKind = {
    id: string
    public: boolean
    default_scope: string[] | null
    listing: { name: string; description: string; icon: string; fields: KindField[] }
}

export type CreatedTrigger = { trigger: Trigger; secret?: string | null }
export type TriggerSummary = { total: number; active: number; needs_person: number }
export type TriggerRunDetail = { run: TriggerRun; events: TriggerEvent[]; history: ChatMessage[] }
export type CheckOutcome = { value?: unknown; error?: string | null; calls?: string[] }

const base = (agentId: string) => `/agents/${encodeURIComponent(agentId)}/triggers`
const one = (agentId: string, slug: string) => `${base(agentId)}/${encodeURIComponent(slug)}`

export const triggerKinds = () => api.get<TriggerKind[]>('/trigger-kinds')
/** Unpaged: an agent has only a few. */
export const listTriggers = (agentId: string, signal?: AbortSignal) => api.get<Trigger[]>(base(agentId), signal)
export const triggerSummary = (agentId: string) => api.get<TriggerSummary>(`/agents/${encodeURIComponent(agentId)}/trigger-summary`)
export const getTrigger = (agentId: string, slug: string) => api.get<Trigger>(one(agentId, slug))
export const createTrigger = (agentId: string, input: TriggerInput) => api.post<CreatedTrigger>(base(agentId), input)
export const updateTrigger = (agentId: string, slug: string, input: TriggerInput) => api.put<Trigger>(one(agentId, slug), input)
export const deleteTrigger = (agentId: string, slug: string) => api.delete(one(agentId, slug))
export const approveTrigger = (agentId: string, slug: string, triageModel?: [string, string]) =>
    api.post<CreatedTrigger>(`${one(agentId, slug)}/approve`, { triage_model: triageModel ?? null })
export const replaceTriggerSecret = (agentId: string, slug: string) => api.post<CreatedTrigger>(`${one(agentId, slug)}/secret`)
export const testTrigger = (agentId: string, slug: string) => api.post<CheckOutcome>(`${one(agentId, slug)}/test`)

export const PAGE = 50

const page = (before?: { at: string; id: number }) => {
    const qs = new URLSearchParams({ limit: String(PAGE) })
    if (before) {
        qs.set('before', before.at)
        qs.set('before_id', String(before.id))
    }
    return qs.toString()
}

export const triggerEvents = (agentId: string, slug: string, before?: TriggerEvent) =>
    api.get<TriggerEvent[]>(`${one(agentId, slug)}/events?${page(before && { at: before.received_at, id: before.id })}`)
export const triggerRuns = (agentId: string, slug: string, before?: TriggerRun) =>
    api.get<TriggerRun[]>(`${one(agentId, slug)}/runs?${page(before && { at: before.started_at, id: before.id })}`)
export const triggerRun = (agentId: string, slug: string, id: number) => api.get<TriggerRunDetail>(`${one(agentId, slug)}/runs/${id}`)
