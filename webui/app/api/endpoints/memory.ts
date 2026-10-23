import { api, request } from '../client'
import type { RevisionActor, RevisionDiff, RevisionTrigger } from '../../lib/revisions'

export interface MemorySummary {
    agent_id: string
    bundle: string
    path: string
    title: string
    created_at: string
    updated_at: string
    tags: string[]
    relations: string[]
    attachment_count: number
}

export interface MemoryDoc extends Omit<MemorySummary, 'attachment_count'> {
    content: string
}

export interface Passage {
    bundle: string
    path: string
    title: string
    ordinal: number
    ordinal_end: number
    line_start: number
    line_end: number
    text: string
    score: number
    truncated: boolean
}

export interface Bundle {
    name: string
    concept_count: number
    updated_at: string | null
    link_count: number
}

export interface GraphNodeDto {
    slug: string
    bundle: string
    title: string
    tags: string[]
    boundary: boolean
}

export interface GraphDto {
    nodes: GraphNodeDto[]
    edges: { source: string; target: string; broken: boolean }[]
}

export interface MemoryRevisionSummary {
    seq: number
    deleted: boolean
    actor: RevisionActor
    trigger: RevisionTrigger
    created_at: string
    is_current: boolean
    size_bytes: number
}

const base = (agentId: string) => `/agents/${agentId}/memory`
const doc = (agentId: string, bundle: string, path: string) =>
    `${base(agentId)}/doc/${encodeURIComponent(bundle)}/${path.split('/').map(encodeURIComponent).join('/')}`
const stripMd = (p: string) => p.replace(/\.md$/, '')

export const searchMemory = (agentId: string, query: string, bundle?: string) =>
    api.get<Passage[]>(`${base(agentId)}/query?query=${encodeURIComponent(query)}&limit=20&threshold=0.2${bundle ? `&bundle=${encodeURIComponent(bundle)}` : ''}`)

/** A bundle's documents, newest first, or by path with `bySlug` (the explorer's order). */
export const listDocs = (agentId: string, bundle: string, offset: number, limit: number, bySlug = false) =>
    api.get<{ memories: MemorySummary[]; total: number; offset: number; limit: number }>(
        `${base(agentId)}?bundle=${encodeURIComponent(bundle)}&offset=${offset}&limit=${limit}${bySlug ? '&sort_by=slug' : ''}`
    )

/** Every bundle, unpaged, for the explorer. */
export const listBundles = (agentId: string) => api.get<Bundle[]>(`${base(agentId)}/bundles`)

export const getDoc = (agentId: string, bundle: string, path: string) => api.get<MemoryDoc>(doc(agentId, bundle, stripMd(path)))

export const saveDoc = (agentId: string, bundle: string, path: string, body: { title: string; content: string; tags: string[] }) =>
    api.put(doc(agentId, bundle, stripMd(path)), body)

export const deleteDoc = (agentId: string, bundle: string, path: string) => api.delete(doc(agentId, bundle, stripMd(path)))

export const createDoc = (agentId: string, body: { title: string; content: string; bundle: string; path?: string; tags: string[] }) =>
    api.post<{ bundle: string; path: string }>(base(agentId), body)

export const bundleGraph = (agentId: string, bundle: string) => api.get<GraphDto>(`${base(agentId)}/${encodeURIComponent(bundle)}/graph`)
export const bundlesGraph = (agentId: string) => api.get<GraphDto>(`${base(agentId)}/bundles/graph`)
export const deleteBundle = (agentId: string, bundle: string, force: boolean) =>
    api.delete(`${base(agentId)}/bundles/${encodeURIComponent(bundle)}?force=${force}`)

export const memoryHistory = (agentId: string, bundle: string, path: string, offset = 0, limit = 20) =>
    api.get<{ revisions: MemoryRevisionSummary[]; total: number }>(
        `${base(agentId)}/history/${encodeURIComponent(bundle)}/${stripMd(path)}?offset=${offset}&limit=${limit}`
    )
export const memoryDiff = (agentId: string, bundle: string, path: string, to: number, from?: number) =>
    api.get<RevisionDiff>(`${base(agentId)}/history/${encodeURIComponent(bundle)}/${stripMd(path)}?to=${to}${from != null ? `&from=${from}` : ''}`)
/** One saved version; `content` is null on a deletion. */
export const memoryRevision = (agentId: string, bundle: string, path: string, seq: number) =>
    api.get<{ seq: number; content: string | null }>(`${base(agentId)}/history/${encodeURIComponent(bundle)}/${stripMd(path)}?seq=${seq}`)
export const rollbackDoc = (agentId: string, bundle: string, path: string, seq: number) =>
    api.post(`${base(agentId)}/history/${encodeURIComponent(bundle)}/${stripMd(path)}`, { seq })

/** The bundle as a `.zip`, fetched with the token so it can be saved. */
export async function exportBundle(agentId: string, bundle: string): Promise<Blob> {
    const { getToken, API_BASE } = await import('../client')
    const res = await fetch(`${API_BASE}${base(agentId)}/bundles/${encodeURIComponent(bundle)}/export`, { headers: { Authorization: `Bearer ${getToken()}` } })
    if (!res.ok) throw new Error(`export failed (${res.status})`)
    return res.blob()
}

export async function importBundle(agentId: string, file: File): Promise<{ imported: string[]; skipped: string[] }> {
    const form = new FormData()
    form.append('file', file)
    return request('POST', `${base(agentId)}/bundles/import`, form)
}
