import { api } from '../client'

export type ProviderSummary = {
    id: string
    name: string
    origin: 'catalogue' | 'builtin' | 'custom'
    kind: string
    dialect?: string | null
    logo?: string | null
    /** The logo has its own colours; the others are drawn in the text colour. */
    logo_full_color?: boolean
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
    logo?: string | null
    logo_full_color?: boolean
    release_date?: string | null
    knowledge?: string | null
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

/** One of OpenRouter's media model capabilities (spec 028). */
export type MediaCapability = 'speech' | 'transcription' | 'image'

/** One entry in a capability's list: the model plus whether it is the documented default. */
export type MediaModelSummary = {
    id: string
    name: string
    default: boolean
    /** False only for a documented default injected because the fetched list no longer has it. */
    listed: boolean
    voices: string[]
    aspect_ratios: string[]
    resolutions: string[]
}

/** One capability's listing and refresh status. */
export type MediaModels = {
    capability: MediaCapability
    source: 'bundled' | 'cached'
    fetched_at: string
    checked_at?: string | null
    default_voice?: string | null
    refresh: {
        enabled: boolean
        ttl: string
        in_progress: boolean
        last_error?: string | null
        failed_at?: string | null
    }
    models: MediaModelSummary[]
}

export const mediaModels = (capability: MediaCapability) =>
    api.get<MediaModels>(`/catalogue/media-models?capability=${capability}`)

/** A person-triggered refresh; needs `settings:providers`. */
export const refreshMediaModels = (capability: MediaCapability) =>
    api.post<MediaModels>(`/catalogue/media-models/${capability}/refresh`)
