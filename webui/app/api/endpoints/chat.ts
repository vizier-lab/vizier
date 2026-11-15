import { api, paged, request, type Page } from '../client'
import type { components } from '../schema'
import type {
    ChatMessage,
    DetachedScriptDetail,
    DetachedScriptSnapshot,
    ModelOverride,
    SessionFileRecord,
    SubagentSnapshot,
    Topic,
} from '../frames'

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

/** The topic's detached scripts: the running ones, then the last 50 that ended. Read-only. */
export const topicDetachedScripts = (agentId: string, topicId: string, channel: string = CHANNEL) =>
    api.get<DetachedScriptSnapshot[]>(`${base(agentId, topicId, channel)}/detached-scripts`)

/** One script with its code, its calls and, once final, its report. */
export const detachedScript = (agentId: string, topicId: string, scriptId: string, channel: string = CHANNEL) =>
    api.get<DetachedScriptDetail>(
        `${base(agentId, topicId, channel)}/detached-scripts/${encodeURIComponent(scriptId)}`
    )

/** The topic's workers: the running and asking ones, then the last 50 that ended. Read-only. */
export const topicSubagents = (agentId: string, topicId: string, channel: string = CHANNEL) =>
    api.get<SubagentSnapshot[]>(`${base(agentId, topicId, channel)}/subagents`)

/** One worker with its whole task and result. */
export const subagent = (agentId: string, topicId: string, id: string, channel: string = CHANNEL) =>
    api.get<SubagentSnapshot>(`${base(agentId, topicId, channel)}/subagents/${encodeURIComponent(id)}`)

/** A worker's own conversation, in stored order: its steps. */
export const subagentHistory = (agentId: string, topicId: string, id: string, channel: string = CHANNEL) =>
    api.get<ChatMessage[]>(`${base(agentId, topicId, channel)}/subagents/${encodeURIComponent(id)}/history`)

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

export type ChatModelOverride = ModelOverride
export type RestChatRequest = Omit<components['schemas']['ChatRequest'], 'model_override'> & { model_override?: ModelOverride }

export const postChat = (agentId: string, body: RestChatRequest, signal?: AbortSignal) =>
    request<components['schemas']['ChatResponse']>('POST', `/agents/${encodeURIComponent(agentId)}/chat`, body, { signal })

export type SideChatRequest = components['schemas']['BtwRequest']
export type SideChatResponse = components['schemas']['BtwResponse']

/** Asks one off-the-record question beside a conversation. Nothing is saved except usage. */
export const askSideChat = (agentId: string, body: SideChatRequest, signal?: AbortSignal) =>
    request<SideChatResponse>('POST', `/agents/${agentId}/btw`, body, { signal })
