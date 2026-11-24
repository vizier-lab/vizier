import { api } from '../client'

export type Overview = {
    agents: { total: number; live: number; failed: number }
    cost_today_usd: number
    accounting_since?: string | null
}

export type Rank = 'most_active' | 'mine' | 'live' | 'failed'

export type ActivityRow = { agent_id: string | null; label: string; turns: number[]; live: boolean }

export type Activity = {
    hours: string[]
    rows: ActivityRow[]
    all?: ActivityRow | null
    agent_total: number
    today_by_kind?: { chat: number; scheduled: number; dream_stages: number; background: number } | null
}

export const getOverview = () => api.get<Overview>('/overview')
export const getActivity = (rank: Rank) => api.get<Activity>(`/overview/activity?rank=${rank}`)
