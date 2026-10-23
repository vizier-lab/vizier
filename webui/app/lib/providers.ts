import type { ConnectedProvider, CredentialField, CustomProvider, UsedBy } from '../api/endpoints/providers'
import { usedTotal } from '../api/endpoints/providers'

export const FIELD_LABELS: Record<CredentialField, string> = {
    api_key: 'API key',
    endpoint: 'Endpoint',
    base_url: 'Base URL',
    access_token: 'Access token',
    account_id: 'Account ID',
}

/** Fields shown in the clear (addresses, ids); the rest are secrets. */
export const PLAIN_FIELDS: CredentialField[] = ['endpoint', 'base_url', 'account_id']

/** "3 agents · 41 models", or "not used yet". */
export function usageText(used: UsedBy, models: number): string {
    const agents = usedTotal(used)
    const m = `${models} model${models === 1 ? '' : 's'}`
    return agents === 0 ? (models ? `not used yet · ${m}` : 'not used yet') : `${agents} agent${agents === 1 ? '' : 's'} · ${m}`
}

/** What a row says about how it is connected: "Saved key", "OPENAI_API_KEY", an address. */
export function credentialText(p: ConnectedProvider): string {
    const where = p.endpoint ?? p.base_url
    const parts: string[] = []
    if (p.has_access_token) parts.push(p.account_id ? `Signed in as ${p.account_id}` : 'Signed in')
    else if (p.has_api_key) parts.push(p.source?.startsWith('env:') ? `Key from ${p.source.slice(4)}` : 'Saved key')
    else if (p.fields.includes('api_key')) parts.push('no key')
    if (where) parts.unshift(where)
    return parts.join(' · ') || (p.source ?? 'ready')
}

export function customCredentialText(p: CustomProvider): string {
    const protocol = p.protocol === 'anthropic' ? 'Anthropic-protocol' : 'OpenAI-compatible'
    return `Custom · ${protocol} · ${p.base_url}`
}

/** The warning before a provider is deleted: who loses it. */
export function inUseMessage(agents: { name: string; features: string[] }[]): string {
    return agents.map((a) => `${a.name} (${a.features.join(', ')})`).join(', ')
}
