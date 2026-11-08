// Typed client for contracts/share-api.md. Every call goes through the shared transport (`api`).
import { api, queryString } from './client'
import type { Removals, Snapshot } from '../lib/share/schema'
import { SHARE_LIMITS } from '../lib/share/limits'
import { ShareError } from '../lib/share/serialize'
import { snapshotBytes } from '../lib/share/preview'
import type { SerializeResult } from '../lib/share/serialize'

export type Audience = 'specific' | 'signed_in' | 'public'

export interface CreateShareRequest {
    agent_id: string
    topic_id: string
    title: string
    audience: Audience
    /** Present (1-50 ids) only with `specific`. */
    people?: string[]
    include_trail: boolean
    removals: Removals
    snapshot: Snapshot
}

export interface CreatedShare {
    share_id: string
    url_path: string
    created_at: number
}

export interface ShareListItem {
    share_id: string
    title: string
    agent_id: string
    agent_name: string
    topic_id: string
    audience: Audience
    people_count: number
    include_trail: boolean
    size_bytes: number
    created_at: number
    revoked_at: number | null
    removals: Removals
    /** Only with `all=true`. */
    creator_name?: string
}

/** What `GET /shares/{id}` answers. `snapshot` is hostile: read it with `parseSnapshot`. */
export interface SharedConversation {
    share_id: string
    title: string
    created_at: number
    audience: Audience
    include_trail: boolean
    format_version: number
    snapshot: unknown
}

export interface PersonMatch {
    user_id: string
    username: string
}

export const FALLBACK_TITLE = 'Shared conversation'

export interface ShareChoice {
    agentId: string
    topicId: string
    title: string
    audience: Audience
    people?: string[]
}

/**
 * The request for what the person previewed. Pure: it sends nothing, and it carries the same
 * `removals` object the preview counted and `include_trail` from the snapshot itself.
 */
export function buildCreateRequest(choice: ShareChoice, result: SerializeResult): CreateShareRequest {
    const title = (choice.title.trim() || result.snapshot.title || FALLBACK_TITLE).slice(0, SHARE_LIMITS.titleLength)
    const request: CreateShareRequest = {
        agent_id: choice.agentId,
        topic_id: choice.topicId,
        title,
        audience: choice.audience,
        include_trail: result.snapshot.include_trail,
        removals: { ...result.removals },
        snapshot: result.snapshot,
    }
    if (choice.audience === 'specific') {
        const people = [...new Set(choice.people ?? [])]
        if (people.length < 1) throw new ShareError('people', 'Name at least one person to share with.')
        if (people.length > SHARE_LIMITS.people) throw new ShareError('people', `You can name at most ${SHARE_LIMITS.people} people.`)
        request.people = people
    } else if (choice.people?.length) {
        throw new ShareError('audience', 'Only a share with named people carries a list of people.')
    }
    if (snapshotBytes(result.snapshot) + SHARE_LIMITS.envelopeBytes > SHARE_LIMITS.bodyBytes) {
        throw new ShareError('too_large', 'This conversation is too large to share.')
    }
    return request
}

export const createShare = (request: CreateShareRequest) => api.post<CreatedShare>('/shares', request)

export const getShare = (shareId: string, signal?: AbortSignal) => api.get<SharedConversation>(`/shares/${encodeURIComponent(shareId)}`, signal)

export const listShares = (query: { before?: number | string; limit?: number; all?: boolean } = {}, signal?: AbortSignal) =>
    api.get<ShareListItem[]>(`/shares${queryString({ before: query.before, limit: query.limit, all: query.all ? 'true' : undefined })}`, signal)

export const revokeShare = (shareId: string) => api.delete<void>(`/shares/${encodeURIComponent(shareId)}`)

/** People to name, by username prefix. Two characters at least, as the server insists. */
export const searchPeople = (q: string, signal?: AbortSignal) => {
    const term = q.trim()
    if (term.length < 2) return Promise.resolve<PersonMatch[]>([])
    return api.get<PersonMatch[]>(`/shares/people${queryString({ q: term })}`, signal)
}

/** The path a person opens, from the id the server returned. */
export const sharePath = (shareId: string) => `/share/${encodeURIComponent(shareId)}`
