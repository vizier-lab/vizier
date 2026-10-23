import { api, paged, type Page } from '../client'
import type { BackgroundJobSnapshot, ChatMessage, SessionFileRecord, Topic } from '../frames'

/** The channel every WebUI conversation lives on. */
export const CHANNEL = 'vizier-webui'

const base = (agentId: string, topicId: string) =>
    `/agents/${agentId}/channel/${CHANNEL}/topic/${encodeURIComponent(topicId)}`

export const listTopics = (agentId: string) => api.get<Topic[]>(`/agents/${agentId}/channel/${CHANNEL}/topics`)

export const topicDetail = (agentId: string, topicId: string) => api.get<Topic>(`${base(agentId, topicId)}/detail`)

export interface HistoryCursor {
    before?: string
    before_seq?: number | null
    limit?: number
}

/** Entries in stored order; with `before`, the page ending just before that entry. */
export function topicHistory(agentId: string, topicId: string, cursor: HistoryCursor = {}) {
    const qs = new URLSearchParams()
    if (cursor.before) qs.set('before', cursor.before)
    if (cursor.before_seq != null) qs.set('before_seq', String(cursor.before_seq))
    if (cursor.limit) qs.set('limit', String(cursor.limit))
    return api.get<ChatMessage[]>(`${base(agentId, topicId)}/history?${qs}`)
}

export const topicJobs = (agentId: string, topicId: string) =>
    api.get<BackgroundJobSnapshot[]>(`${base(agentId, topicId)}/jobs`)

export const cancelJob = (agentId: string, topicId: string, jobId: string) =>
    api.post<BackgroundJobSnapshot>(`${base(agentId, topicId)}/jobs/${encodeURIComponent(jobId)}/cancel`, {})

export const topicFiles = (agentId: string, topicId: string, offset = 0, limit = 100) =>
    paged<SessionFileRecord>(`${base(agentId, topicId)}/files`, { offset, limit }) as Promise<Page<SessionFileRecord>>

export const deleteTopic = (agentId: string, topicId: string) => api.delete(base(agentId, topicId))

export const restartAgent = (agentId: string) => api.post(`/agents/${agentId}/restart`)

export function chatSocketUrl(agentId: string, topicId: string, token: string): string {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:'
    return `${proto}//${location.host}/api/v1/agents/${agentId}/channel/${CHANNEL}/topic/${encodeURIComponent(topicId)}/chat?token=${encodeURIComponent(token)}`
}
