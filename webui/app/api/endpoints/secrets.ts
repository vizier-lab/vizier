import { api } from '../client'

/** One secret as the server reports it: a name, its mask, and who changed it when. */
export type SecretItem = {
    name: string
    masked: string
    created_at: string
    updated_at: string
    updated_by: { user: string } | { agent: string }
    updated_by_name: string
}

export type SecretList = { items: SecretItem[] }

/** `created` when the name was new, `replaced` when it existed. */
export type SecretWrite = { status: 'created' | 'replaced'; secret: SecretItem }

export type SecretEvent = {
    id: string
    name: string
    action: 'created' | 'replaced' | 'deleted'
    actor: { user: string } | { agent: string }
    actor_name: string
    at: string
}

export type SecretEventList = { items: SecretEvent[] }

const base = (agentId: string) => `/agents/${encodeURIComponent(agentId)}/secrets`

/** Names, masks and last change. No route returns a value. */
export const listSecrets = (agentId: string) => api.get<SecretList>(base(agentId))

/** Creating and replacing are the same `PUT`: the name is in the path, the body only the value. */
export const createSecret = (agentId: string, name: string, value: string) =>
    api.put<SecretWrite>(`${base(agentId)}/${encodeURIComponent(name)}`, { value })

export const replaceSecret = (agentId: string, name: string, value: string) =>
    createSecret(agentId, name, value)

export const deleteSecret = (agentId: string, name: string) =>
    api.delete<{ name: string; deleted: true }>(`${base(agentId)}/${encodeURIComponent(name)}`)

/** Newest first. The server defaults to 20 and caps at 100. */
export const listSecretEvents = (agentId: string, limit = 20) =>
    api.get<SecretEventList>(`${base(agentId)}/events?limit=${limit}`)
