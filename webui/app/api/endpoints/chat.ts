import { api, paged, request, type Page } from '../client'
import type { components } from '../schema'
import type { BackgroundJobSnapshot, ChatMessage, SessionFileRecord, Topic } from '../frames'

/** The channel every WebUI conversation lives on. */
export const CHANNEL = 'vizier-webui'

const base = (agentId: string, topicId: string, channel: string = CHANNEL) =>
    `/agents/${agentId}/channel/${encodeURIComponent(channel)}/topic/${encodeURIComponent(topicId)}`

export const listTopics = (agentId: string, channel: string = CHANNEL) =>
    api.get<Topic[]>(`/agents/${agentId}/channel/${encodeURIComponent(channel)}/topics`)

export const topicDetail = (agentId: string, topicId: string, channel: string = CHANNEL) =>
    api.get<Topic>(`${base(agentId, topicId, channel)}/detail`)

export interface HistoryCursor {
    /** Read another channel (`discord__123`) instead of the WebUI's own. Read-only. */
    channel?: string
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
    return api.get<ChatMessage[]>(`${base(agentId, topicId, cursor.channel)}/history?${qs}`)
}

export const topicJobs = (agentId: string, topicId: string, channel: string = CHANNEL) =>
    api.get<BackgroundJobSnapshot[]>(`${base(agentId, topicId, channel)}/jobs`)

export const cancelJob = (
    agentId: string,
    topicId: string,
    jobId: string,
    channel: string = CHANNEL
) =>
    api.post<BackgroundJobSnapshot>(
        `${base(agentId, topicId, channel)}/jobs/${encodeURIComponent(jobId)}/cancel`,
        {}
    )

export const topicFiles = (
    agentId: string,
    topicId: string,
    offset = 0,
    limit = 100,
    channel: string = CHANNEL
) =>
    paged<SessionFileRecord>(`${base(agentId, topicId, channel)}/files`, { offset, limit }) as Promise<
        Page<SessionFileRecord>
    >

export const deleteTopic = (agentId: string, topicId: string, channel: string = CHANNEL) =>
    api.delete(base(agentId, topicId, channel))

export const renameTopic = (
    agentId: string,
    topicId: string,
    title: string,
    channel: string = CHANNEL
) => api.patch<Topic>(base(agentId, topicId, channel), { title })

export const restartAgent = (agentId: string) => api.post(`/agents/${agentId}/restart`)

export type SideChatRequest = components['schemas']['BtwRequest']
export type SideChatResponse = components['schemas']['BtwResponse']

/** Asks one off-the-record question beside a conversation. Nothing is saved except usage. */
export const askSideChat = (agentId: string, body: SideChatRequest, signal?: AbortSignal) =>
    request<SideChatResponse>('POST', `/agents/${agentId}/btw`, body, { signal })
