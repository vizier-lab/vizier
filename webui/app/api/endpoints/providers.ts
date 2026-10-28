import { api } from '../client'
import type { ModelSummary } from './catalogue'

export type CredentialField =
    | 'api_key'
    | 'endpoint'
    | 'base_url'
    | 'access_token'
    | 'account_id'
    | 'access_key_id'
    | 'secret_access_key'
    | 'session_token'
    | 'region'

export type UsedBy = { chat: number; stt: number; tts: number; image_gen: number; embedding: number }

export type ConnectedProvider = {
    id: string
    name: string
    origin: string
    kind: string
    model_count: number
    fields: CredentialField[]
    source?: string | null
    has_api_key: boolean
    endpoint?: string | null
    base_url?: string | null
    has_access_token: boolean
    account_id?: string | null
    /** AWS Bedrock: an identifier, safe to show. */
    access_key_id?: string | null
    has_secret_access_key?: boolean
    has_session_token?: boolean
    /** AWS region, safe to show. */
    region?: string | null
    used_by: UsedBy
    reachable?: boolean | null
}

export type CustomProtocol = 'openai_compatible' | 'anthropic'
export type CustomModel = {
    id: string
    context?: number | null
    output?: number | null
    tool_call?: boolean | null
    image_input?: boolean | null
    reasoning?: boolean | null
    edited?: boolean
}

export type CustomProvider = {
    id: string
    name: string
    protocol: CustomProtocol
    base_url: string
    has_api_key: boolean
    headers: string[]
    models: CustomModel[]
    models_fetched_at?: string | null
    used_by: UsedBy
    reachable?: boolean | null
}

export type CustomBody = {
    id?: string
    name: string
    protocol: CustomProtocol
    base_url: string
    api_key?: string
    headers?: Record<string, string | null>
    models?: CustomModel[]
}

export type AffectedAgent = { id: string; name: string; features: string[] }
export type TestResult = { ok: boolean; model_count?: number; status?: number; error?: string }

export type CatalogueStatus = { source: string; fetched_at: string; checked_at?: string | null; provider_count: number; model_count: number; refresh: { enabled: boolean; in_progress: boolean; last_error?: string | null } }

export const usedTotal = (u: UsedBy) => u.chat + u.stt + u.tts + u.image_gen + u.embedding

export const connectedProviders = () => api.get<ConnectedProvider[]>('/providers')
export const customProviders = () => api.get<CustomProvider[]>('/providers/custom')
export const saveProvider = (
    id: string,
    creds: Partial<Record<'api_key' | 'endpoint' | 'base_url' | 'access_token' | 'account_id' | 'access_key_id' | 'secret_access_key' | 'session_token' | 'region', string>>,
) => api.put<ConnectedProvider>(`/providers/${encodeURIComponent(id)}`, creds)
export const deleteProvider = (id: string, confirm = false) => api.delete<string>(`/providers/${encodeURIComponent(id)}${confirm ? '?confirm=true' : ''}`)
export const testProvider = (id: string) => api.post<TestResult>(`/providers/${encodeURIComponent(id)}/test`)
export const createCustom = (body: CustomBody) => api.post<CustomProvider>('/providers/custom', body)
export const updateCustom = (id: string, body: CustomBody) => api.put<CustomProvider>(`/providers/custom/${encodeURIComponent(id)}`, body)
export const deleteCustom = (id: string, confirm = false) => api.delete<string>(`/providers/custom/${encodeURIComponent(id)}${confirm ? '?confirm=true' : ''}`)
export const testCustom = (body: CustomBody) => api.post<TestResult>('/providers/custom/test', body)
export const fetchCustomModels = (id: string) => api.post<CustomProvider>(`/providers/custom/${encodeURIComponent(id)}/fetch-models`)
export const catalogueStatus = () => api.get<CatalogueStatus>('/catalogue')
export const refreshCatalogue = () => api.post<CatalogueStatus>('/catalogue/refresh')
export const allModels = (id: string) => api.get<ModelSummary[]>(`/catalogue/providers/${encodeURIComponent(id)}/models?all=true`)
