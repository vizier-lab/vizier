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

export type FeedEvent = {
    id: string
    at: string
    agent_id: string
    kind: 'start_failed' | 'task_run' | 'dream_cycle' | 'background_job' | 'integration' | 'channel_replies'
    tone: 'failed' | 'default'
    text: string
    href: string
}

export type Feed = { since: string; events: FeedEvent[]; next?: string | null }

/** The person's UTC offset as the API reads it: `+07:00`, `-05:30`, `+00:00`. */
export function tzParam(date: Date = new Date()): string {
    const minutes = -date.getTimezoneOffset()
    const sign = minutes < 0 ? '-' : '+'
    const abs = Math.abs(minutes)
    return `${sign}${String(Math.floor(abs / 60)).padStart(2, '0')}:${String(abs % 60).padStart(2, '0')}`
}

const tz = () => encodeURIComponent(tzParam())

export const getOverview = () => api.get<Overview>(`/overview?tz=${tz()}`)
export const getActivity = (rank: Rank) => api.get<Activity>(`/overview/activity?rank=${rank}&tz=${tz()}`)
export const getFeed = (before?: string, limit = 20) =>
    api.get<Feed>(`/overview/feed?limit=${limit}${before ? `&before=${encodeURIComponent(before)}` : ''}`)
