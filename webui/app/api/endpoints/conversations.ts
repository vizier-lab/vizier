import { api } from '../client'

export type ConversationKind = 'webui' | 'discord' | 'telegram'

export type ConversationItem = {
    channel: string
    kind: ConversationKind
    title: string
    place: string
    topics: number
    last_activity: string
    /** The WebUI topic to open; a Discord or Telegram conversation has none. */
    latest_topic: string
}

export type ConversationPage = {
    items: ConversationItem[]
    counts: { all: number; webui: number; discord: number; telegram: number }
    total: number
    next?: string | null
}

export function listConversations(agentId: string, query: { q?: string; channel?: ConversationKind | 'all'; cursor?: string; limit?: number }, signal?: AbortSignal) {
    const qs = new URLSearchParams()
    if (query.q) qs.set('q', query.q)
    if (query.channel && query.channel !== 'all') qs.set('channel', query.channel)
    if (query.cursor) qs.set('cursor', query.cursor)
    qs.set('limit', String(query.limit ?? 25))
    return api.get<ConversationPage>(`/agents/${encodeURIComponent(agentId)}/conversations?${qs}`, signal)
}
