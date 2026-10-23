import { api } from '../client'

export type IntegrationFamily = 'channel' | 'addon' | 'mcp'

export type IntegrationState =
    | { state: 'disabled' }
    | { state: 'incomplete'; missing: string[] }
    | { state: 'connecting' }
    | { state: 'running'; tools?: number | null }
    | { state: 'reconnecting'; last_error: string; next_retry_at?: string | null }
    | { state: 'failed'; last_error: string }
    | { state: 'unreachable'; last_error: string }
    | { state: 'unknown' }

export type IntegrationItem = {
    id: string
    kind: string
    family: IntegrationFamily
    name: string
    enabled: boolean
    creator_name?: string | null
    settings: Record<string, unknown>
    missing?: string[]
    status: IntegrationState
    can_edit: boolean
    legacy_linked?: { agents: { agent_id: string; name: string }[]; others: number }
}

export type IntegrationList = {
    family?: IntegrationFamily | null
    can_manage: boolean
    items: IntegrationItem[]
}

export type KindField = { key: string; label: string; type: string; required?: boolean; default?: unknown; options?: string[] }
export type Kind = { id: string; family: IntegrationFamily; name: string; description: string; icon: string; fields: KindField[] }

export type GrantState = { on: boolean; granted_by?: string | null; granted_at?: string | null }
export type Grants = { can_manage: boolean; grants: Record<IntegrationFamily, GrantState> }

export type IntegrationEvent = {
    id: string
    integration_id?: string | null
    integration_name: string
    family: IntegrationFamily
    action: 'created' | 'edited' | 'enabled' | 'disabled' | 'deleted' | 'unlinked' | 'grant_on' | 'grant_off'
    fields: string[]
    actor_name: string
    at: string
}
export type EventPage = { items: IntegrationEvent[]; next?: string | null }

const base = (agentId: string) => `/agents/${encodeURIComponent(agentId)}/integrations`

export const listIntegrations = (agentId: string, family: IntegrationFamily) =>
    api.get<IntegrationList>(`${base(agentId)}?family=${family}`)
export const createIntegration = (agentId: string, body: { kind: string; name: string; settings: unknown; enabled?: boolean }) =>
    api.post<IntegrationItem>(base(agentId), body)
export const updateIntegration = (id: string, body: { name?: string; settings?: unknown; enabled?: boolean }) =>
    api.put<IntegrationItem>(`/integrations/${encodeURIComponent(id)}`, body)
export const removeIntegration = (agentId: string, id: string) =>
    api.delete<{ outcome: 'deleted' | 'unlinked' }>(`${base(agentId)}/${encodeURIComponent(id)}`)
export const integrationKinds = (family: IntegrationFamily) => api.get<Kind[]>(`/integrations/kinds?family=${family}`)
export const getGrants = (agentId: string) => api.get<Grants>(`${base(agentId)}/grants`)
export const setGrant = (agentId: string, family: IntegrationFamily, on: boolean) =>
    api.put<Grants>(`${base(agentId)}/grants`, { [family]: on })
export const integrationEvents = (agentId: string, before?: string, limit = 20) =>
    api.get<EventPage>(`${base(agentId)}/events?limit=${limit}${before ? `&before=${encodeURIComponent(before)}` : ''}`)
