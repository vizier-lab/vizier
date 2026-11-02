import type { ModelSummary } from '../api/endpoints/catalogue'
import type {
    ConnectedProvider,
    CredentialField,
    CustomProvider,
    EntryPrice,
    EntryPricing,
    ProfileEntry,
    UsedBy,
} from '../api/endpoints/providers'
import { usedTotal } from '../api/endpoints/providers'

export const FIELD_LABELS: Record<CredentialField, string> = {
    api_key: 'API key',
    endpoint: 'Endpoint',
    base_url: 'Base URL',
    access_token: 'Access token',
    account_id: 'Account ID',
    access_key_id: 'Access key ID',
    secret_access_key: 'Secret access key',
    session_token: 'Session token (optional)',
    region: 'Region',
    start_url: 'Identity Center start URL',
    sso_region: 'SSO region',
    role_name: 'Role name',
    sso_profile: 'Profile name',
}

/** Fields shown in the clear (addresses, ids); the rest are secrets. */
export const PLAIN_FIELDS: CredentialField[] = [
    'endpoint',
    'base_url',
    'account_id',
    'access_key_id',
    'region',
    // The Identity Center connection details are addresses and names, never material
    // (spec 044, contracts/provider-credentials-api.md §1).
    'start_url',
    'sso_region',
    'role_name',
    'sso_profile',
]

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
    else if (p.has_secret_access_key) parts.push(p.source?.startsWith('env:') ? `Keys from ${p.source.slice(4)}` : 'Saved keys')
    else if (p.access_key_id) parts.push('no secret key')
    else if (p.has_api_key) parts.push(p.source?.startsWith('env:') ? `Key from ${p.source.slice(4)}` : 'Saved key')
    else if (p.fields.includes('api_key')) parts.push('no key')
    if (p.region) parts.push(p.region)
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

// ---- Inference-profile entries (spec 044, FR-034a, SC-021)
//
// The entry's wire shape and calls live in `api/endpoints/providers.ts`; everything the settings row
// makes of them lives here, so the form and the drawer share one shape, one phrase and one wording
// (rule W5). The reason a price is missing is the server's, rendered verbatim: this module never
// invents wording and never substitutes a fallback for an absence it was not given (SC-021).

/** The connection that carries inference-profile entries (spec 044, FR-022). */
export const SSO_PROVIDER = 'amazon-bedrock-sso'

/** Catalogue rows that may price an entry: the ARN entries the same list carries are not models (contracts §5a). */
export function pricingCandidates(models: ModelSummary[]): ModelSummary[] {
    return models.filter((m) => m.origin !== 'inference_profile')
}

/** A limit as the API takes it: `null` when empty, `NaN` when it is not a positive whole number. */
export function entryLimit(value: string): number | null {
    const text = value.trim()
    if (!text) return null
    const n = Number(text)
    return Number.isSafeInteger(n) && n > 0 ? n : Number.NaN
}

/** A price rate: `null` when empty, `NaN` when it is not a finite, non-negative number. */
export function entryRate(value: string): number | null {
    const text = value.trim()
    if (!text) return null
    const n = Number(text)
    return Number.isFinite(n) && n >= 0 ? n : Number.NaN
}

/** What the form's two price fields amount to. */
export type EntryPriceDraft = { price: EntryPrice | null; touched: boolean; invalid: boolean }

/** The entry's price from the form's two fields: an empty pair is no price, an unusable pair is invalid. */
export function draftPrice(input: string, output: string, kept?: EntryPrice | null): EntryPriceDraft {
    const touched = input.trim() !== '' || output.trim() !== ''
    if (!touched) return { price: null, touched, invalid: false }
    const i = entryRate(input)
    const o = entryRate(output)
    if (i === null || o === null || Number.isNaN(i) || Number.isNaN(o)) return { price: null, touched, invalid: true }
    return {
        price: { input: i, output: o, cache_read: kept?.cache_read ?? null, cache_write: kept?.cache_write ?? null },
        touched,
        invalid: false,
    }
}

/** An entry's limits as one phrase; an unset one is unknown, never "unsupported" (FR-028). */
export function entryLimits(entry: Pick<ProfileEntry, 'context' | 'output'>): string {
    return [
        entry.context ? `${entry.context.toLocaleString()} context` : 'context unknown',
        entry.output ? `${entry.output.toLocaleString()} output` : 'output unknown',
    ].join(' · ')
}

/** A rate in the catalogue's dollars, at the precision the figure needs. */
const usd = (n: number) => `$${n.toFixed(n < 1 ? 3 : 2)}`

/**
 * What a row says about an entry's price. The two shapes are disjoint on purpose: a priced note
 * carries the figure and **no** absence wording, an unpriced one carries the label **and** the
 * server's cause-specific reason, so a bare label has nowhere to live (contracts §4a, SC-021).
 */
export type EntryPricingNote = { priced: true; text: string } | { priced: false; flag: string; reason: string }

/** The price note for an entry, from the `pricing` state the API computed for this read. */
export function pricingNote(pricing: EntryPricing): EntryPricingNote {
    if (pricing.state === 'explicit' || pricing.state === 'from_catalogue') {
        const source = pricing.state === 'explicit' ? (pricing.source ?? 'entered price') : `from ${pricing.model}`
        return { priced: true, text: `${usd(pricing.cost.input)} in / ${usd(pricing.cost.output)} out per 1M · ${source}` }
    }
    return { priced: false, flag: pricing.flag, reason: pricing.reason }
}
