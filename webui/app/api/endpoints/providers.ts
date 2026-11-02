import { api, queryString } from '../client'
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
    // Amazon Bedrock over IAM Identity Center (spec 044): all four are shown in clear.
    | 'start_url'
    | 'sso_region'
    | 'role_name'
    | 'sso_profile'

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
    // Amazon Bedrock over IAM Identity Center (spec 044, contracts §4). Non-secret only:
    // the session itself never leaves the server.
    sso_state?: SsoSessionState | null
    /** A session row exists and the server can read it. */
    has_sso_session?: boolean
    /** The non-secret name the session is stored under. */
    sso_profile?: string | null
    /** The Identity Center start URL, safe to show. */
    start_url?: string | null
    /** The Identity Center's region — not the Bedrock region. */
    sso_region?: string | null
    /** The role chosen from the portal. */
    role_name?: string | null
    /** When the stored session's access token expires, ISO 8601. */
    session_expires_at?: string | null
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
    creds: Partial<Record<CredentialField, string>>,
) => api.put<ConnectedProvider>(`/providers/${encodeURIComponent(id)}`, creds)
export const deleteProvider = (id: string, confirm = false) => api.delete<string>(`/providers/${encodeURIComponent(id)}${confirm ? '?confirm=true' : ''}`)
export const testProvider = (id: string) => api.post<TestResult>(`/providers/${encodeURIComponent(id)}/test`)
// ---- Amazon Bedrock over IAM Identity Center (spec 044, contracts/provider-credentials-api.md §3)

/** `configured`, `signed-in`, `expired`, or a session row this workspace key cannot read. */
export type SsoSessionState = 'configured' | 'signed-in' | 'expired' | 'unreadable'

/** Every member is optional: an omitted value falls back to the saved connection (FR-008). */
export type SignInRequest = { start_url?: string; sso_region?: string; profile?: string }

/** The pending panel: the code to type and where to type it — never the device code. */
export type SignInStarted = {
    user_code: string
    verification_uri: string
    verification_uri_complete?: string
    expires_at: string
    interval_seconds: number
}

/** The answer to a poll: still waiting, or done. */
export type SignInProgress =
    | { state: 'pending'; expires_at: string }
    | { state: 'signed-in'; account_id?: string; role_name?: string; expires_at: string }

/** The three bodies §3 shows, untagged on the wire. `'user_code' in r` tells the code apart. */
export type SignInResponse = SignInStarted | SignInProgress

export type SsoAccount = { account_id: string; account_name: string; email?: string }
/** `fallback` means the portal listed nothing: offer free text instead of a picker (FR-007). */
export type AccountList = { accounts: SsoAccount[]; fallback: boolean }
export type SsoRole = { role_name: string; account_id: string }
export type RoleList = { roles: SsoRole[]; fallback: boolean }
export type SignOutResponse = { state: SsoSessionState }

/** Starts or continues a sign-in; `202` (still pending) arrives as a `pending` answer, not an error. */
export const ssoSignIn = (id: string, body: SignInRequest = {}) =>
    api.post<SignInResponse>(`/providers/${encodeURIComponent(id)}/sso/sign-in`, body)

export const ssoAccounts = (id: string, query: SignInRequest = {}, signal?: AbortSignal) =>
    api.get<AccountList>(`/providers/${encodeURIComponent(id)}/sso/accounts${queryString(query)}`, signal)

export const ssoRoles = (id: string, accountId: string, query: SignInRequest = {}, signal?: AbortSignal) =>
    api.get<RoleList>(`/providers/${encodeURIComponent(id)}/sso/accounts/${encodeURIComponent(accountId)}/roles${queryString(query)}`, signal)

export const ssoSignOut = (id: string, body: SignInRequest = {}) =>
    api.post<SignOutResponse>(`/providers/${encodeURIComponent(id)}/sso/sign-out`, body)

// ---- Inference-profile entries (spec 044, contracts/provider-credentials-api.md §4-§5)
//
// The entry's shape and the calls that carry it live here with the other provider calls; what the
// settings row makes of them — the limits line, the price note and the reason an absence carries —
// is `lib/providers.ts`, so no component holds a second copy (rule W5).

/** USD per 1M tokens; `cache_*` are carried through untouched, never invented. */
export type EntryPrice = { input: number; output: number; cache_read?: number | null; cache_write?: number | null }

/** What the entry's usage costs right now (contracts §4a). Every absence carries its own reason. */
export type EntryPricing =
    | { state: 'explicit'; cost: EntryPrice; source?: string }
    | { state: 'from_catalogue'; cost: EntryPrice; model: string }
    | { state: 'not_resolved' | 'not_set' | 'no_catalogue'; flag: string; reason: string; model?: string }

/** One saved inference-profile entry as the API returns it (contracts §4). */
export type ProfileEntry = {
    arn: string
    name?: string | null
    context?: number | null
    output?: number | null
    tool_call?: boolean | null
    image_input?: boolean | null
    model?: string | null
    price?: EntryPrice | null
    edited?: boolean
    display_name: string
    pricing: EntryPricing
}

/** The whole entry, as the entry endpoints take it. */
export type ProfileEntryBody = {
    arn: string
    name: string | null
    context: number | null
    output: number | null
    tool_call: boolean | null
    image_input: boolean | null
    model: string | null
    price: EntryPrice | null
    edited: boolean
}

export const profileEntries = (id: string) => api.get<ProfileEntry[]>(`/providers/${encodeURIComponent(id)}/profiles`)

export const addProfileEntry = (id: string, body: ProfileEntryBody) =>
    api.post<ProfileEntry[]>(`/providers/${encodeURIComponent(id)}/profiles`, body)

/** Replaces the entry saved under `arn`; the ARN rides in the path so a person can verify it. */
export const saveProfileEntry = (id: string, arn: string, body: ProfileEntryBody, confirm = false) =>
    api.put<ProfileEntry[]>(`/providers/${encodeURIComponent(id)}/profiles/${encodeURIComponent(arn)}${confirm ? '?confirm=true' : ''}`, body)

export const removeProfileEntry = (id: string, arn: string, confirm = false) =>
    api.delete<ProfileEntry[]>(`/providers/${encodeURIComponent(id)}/profiles/${encodeURIComponent(arn)}${confirm ? '?confirm=true' : ''}`)

export const createCustom = (body: CustomBody) => api.post<CustomProvider>('/providers/custom', body)
export const updateCustom = (id: string, body: CustomBody) => api.put<CustomProvider>(`/providers/custom/${encodeURIComponent(id)}`, body)
export const deleteCustom = (id: string, confirm = false) => api.delete<string>(`/providers/custom/${encodeURIComponent(id)}${confirm ? '?confirm=true' : ''}`)
export const testCustom = (body: CustomBody) => api.post<TestResult>('/providers/custom/test', body)
export const fetchCustomModels = (id: string) => api.post<CustomProvider>(`/providers/custom/${encodeURIComponent(id)}/fetch-models`)
export const catalogueStatus = () => api.get<CatalogueStatus>('/catalogue')
export const refreshCatalogue = () => api.post<CatalogueStatus>('/catalogue/refresh')
export const allModels = (id: string) => api.get<ModelSummary[]>(`/catalogue/providers/${encodeURIComponent(id)}/models?all=true`)
