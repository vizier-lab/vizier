import { api, request } from '../client'
import type { RevisionActor, RevisionDiff, RevisionTrigger } from '../../lib/revisions'

export interface CoreContent {
    content: string
    seq?: number
}

export interface CoreRevisionSummary {
    seq: number
    actor: RevisionActor
    trigger: RevisionTrigger
    created_at: string
    is_current: boolean
    size_bytes: number
}

export interface PaginatedCoreRevisions {
    revisions: CoreRevisionSummary[]
    total: number
    offset: number
    limit: number
}

export const getCore = (agentId: string) => api.get<CoreContent>(`/agents/${agentId}/core`)

/** Saves CORE; `base_seq` makes a save over a newer revision a `stale_revision` conflict. */
export const saveCore = (agentId: string, content: string, baseSeq?: number) =>
    api.put<{ message: string; seq?: number }>(`/agents/${agentId}/core`, { content, base_seq: baseSeq })

export const coreHistory = (agentId: string, offset = 0, limit = 20) =>
    api.get<PaginatedCoreRevisions>(`/agents/${agentId}/core/history?offset=${offset}&limit=${limit}`)

export const coreRevision = (agentId: string, seq: number) =>
    api.get<{ seq: number; content: string }>(`/agents/${agentId}/core/history?seq=${seq}`)

export const coreDiff = (agentId: string, to: number, from?: number) =>
    api.get<RevisionDiff>(`/agents/${agentId}/core/history?to=${to}${from != null ? `&from=${from}` : ''}`)

export const rollbackCore = (agentId: string, seq: number) =>
    api.post<{ no_change: boolean; new_seq: number | null; restored_from: number }>(`/agents/${agentId}/core/history`, { seq })

export { request }
