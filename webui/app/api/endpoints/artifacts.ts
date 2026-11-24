import { api } from '../client'
import { transport } from '../transport'

export type ArtifactKind = 'image' | 'pdf' | 'text' | 'markdown' | 'csv' | 'audio' | 'video' | 'other'
export type ArtifactAction = 'promoted' | 'replaced' | 'deleted'

export interface ArtifactItem {
    id: string
    agent_id: string
    name: string
    kind: ArtifactKind
    mime_type: string
    size: number
    version: number
    created_at: string
    updated_at: string
    available: boolean
}

export interface ArtifactPage {
    items: ArtifactItem[]
    next?: string | null
    total: number
    total_bytes: number
    agent_total: number
    agent_total_bytes: number
}

export interface ArtifactReference {
    kind: 'message' | 'memory' | string
    label: string
    href?: string | null
    created_at: string
}

export interface ArtifactEvent {
    id: string
    artifact_id: string
    artifact_name: string
    action: ArtifactAction
    actor: { user: string } | { agent: string }
    actor_name: string
    source_channel?: string | null
    source_topic?: string | null
    source_filename?: string | null
    created_at: string
}

export interface ArtifactDetail {
    artifact: ArtifactItem
    origin: { channel?: string | null; topic?: string | null; source_filename?: string | null }
    available: boolean
    references: ArtifactReference[]
    events: ArtifactEvent[]
}

export interface ArtifactActivity {
    items: ArtifactEvent[]
    next?: string | null
}

export interface ArtifactListParams {
    q?: string
    kind?: ArtifactKind | 'all'
    from?: string
    to?: string
    cursor?: string
    limit?: number
}

const base = (agentId: string) => `/agents/${agentId}/artifacts`

export const listArtifacts = (agentId: string, params: ArtifactListParams) => {
    const qs = new URLSearchParams()
    if (params.q) qs.set('q', params.q)
    if (params.kind && params.kind !== 'all') qs.set('kind', params.kind)
    if (params.from) qs.set('from', params.from)
    if (params.to) qs.set('to', params.to)
    if (params.cursor) qs.set('cursor', params.cursor)
    if (params.limit) qs.set('limit', String(params.limit))
    return api.get<ArtifactPage>(`${base(agentId)}?${qs}`)
}

export const getArtifact = (agentId: string, id: string) =>
    api.get<ArtifactDetail>(`${base(agentId)}/${encodeURIComponent(id)}`)

export const artifactActivity = (agentId: string, cursor?: string, limit = 50) => {
    const qs = new URLSearchParams({ limit: String(limit) })
    if (cursor) qs.set('cursor', cursor)
    return api.get<ArtifactActivity>(`${base(agentId)}/activity?${qs}`)
}

export const deleteArtifact = (agentId: string, id: string) =>
    api.delete<{ removed: boolean; references: number; memory_links_removed: number }>(
        `${base(agentId)}/${encodeURIComponent(id)}`
    )

/** The content URL used by `<img>`/`<audio>`/`<video>`/`<iframe>`, with the auth token. */
export function artifactContentUrl(agentId: string, artifactId: string): string {
    return transport.assetUrl(`/agents/${agentId}/artifacts/${encodeURIComponent(artifactId)}/content`)
}

export const promoteTopicFile = (
    agentId: string,
    channelId: string,
    topicId: string,
    body: { filename: string; name?: string }
) =>
    api.post<ArtifactItem>(
        `/agents/${agentId}/channel/${encodeURIComponent(channelId)}/topic/${encodeURIComponent(topicId)}/files/promote`,
        body
    )

export const clearTopicFiles = (agentId: string, channelId: string, topicId: string) =>
    api.delete<{ removed: number; bytes: number }>(
        `/agents/${agentId}/channel/${encodeURIComponent(channelId)}/topic/${encodeURIComponent(topicId)}/files`
    )
