import { api } from '../client'

export type ProviderSummary = {
    id: string
    name: string
    origin: 'catalogue' | 'builtin' | 'custom'
    kind: string
    dialect?: string | null
    logo?: string | null
    model_count: number
    connection: { status: 'connected' | 'missing' | 'unreachable'; source?: string | null }
    popular: boolean
    fields: ('api_key' | 'endpoint' | 'base_url' | 'access_token' | 'account_id')[]
    endpoint?: string | null
    doc?: string | null
}

export type ModelSummary = {
    id: string
    name: string
    default: boolean
    limit: { context?: number | null; input?: number | null; output?: number | null }
    caps: { tool_call?: boolean | null; reasoning?: boolean | null; image_input?: boolean | null; attachment?: boolean | null }
    cost?: { input: number; output: number; cache_read?: number | null; cache_write?: number | null } | null
    status: 'active' | 'beta' | 'alpha' | 'deprecated'
    origin?: string
}

/** Providers an agent can run on: everything but the media-only ones (speech, embeddings). */
export const chatProviders = () =>
    api.get<ProviderSummary[]>('/catalogue/providers').then((all) => (all ?? []).filter((p) => p.kind !== 'media'))
export const providerModels = (id: string) => api.get<ModelSummary[]>(`/catalogue/providers/${encodeURIComponent(id)}/models`)

/** Every provider the catalogue lists, for the "Add a provider" grid. */
export const allProviders = () => api.get<ProviderSummary[]>('/catalogue/providers')
