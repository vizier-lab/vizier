import { api } from '../client'

export type CycleState = 'running' | 'complete' | 'failed'

export interface DreamChange {
    kind: 'wrote' | 'revised' | 'removed' | 'core'
    bundle: string | null
    path: string | null
    seq: number
    at: string
}

export interface DreamCycle {
    cycle_id: string
    started_at: string
    ended_at: string | null
    state: CycleState
    sessions_read: number
    sources: string[]
    written: number
    revised: number
    removed: number
    /** Did this cycle change the agent's identity? */
    core: boolean
    /** How many identity changes — the reserved bundle's revisions, counted on their own. */
    core_changes: number
    changes: DreamChange[]
    more: number
    model: string | null
    cost_usd: number | null
}

export interface DreamNight {
    date: string
    cycle_id: string | null
    /** written + revised + removed, excluding identity changes. */
    memories: number
    core: boolean
    core_changes: number
    state: CycleState | null
}

export interface DreamStatus {
    status: string
    total_sessions?: number | null
    completed_sessions?: number | null
    last_dream?: string | null
    next_dream?: string | null
    dream_provider?: string | null
    dream_model?: string | null
}

/** Minutes east of UTC as `+07:00`, for the server's night boundaries. */
export const utcOffset = (now = new Date()) => {
    const m = -now.getTimezoneOffset()
    const sign = m < 0 ? '-' : '+'
    const a = Math.abs(m)
    return `${sign}${String(Math.floor(a / 60)).padStart(2, '0')}:${String(a % 60).padStart(2, '0')}`
}

const base = (agentId: string) => `/agents/${agentId}/dream`

export const dreamNights = (agentId: string, days = 30) =>
    api.get<{ nights: DreamNight[]; unattributed: number }>(`${base(agentId)}/nights?days=${days}&tz=${encodeURIComponent(utcOffset())}`)

export const dreamCycles = (agentId: string, before?: string, limit = 20) =>
    api.get<{ cycles: DreamCycle[]; next: string | null }>(`${base(agentId)}/cycles?limit=${limit}${before ? `&before=${encodeURIComponent(before)}` : ''}`)

export const dreamStatus = (agentId: string) => api.get<DreamStatus>(`${base(agentId)}/status`)
export const triggerDream = (agentId: string) => api.post<string>(`${base(agentId)}/trigger`)
