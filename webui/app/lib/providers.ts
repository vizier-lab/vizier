import type { ModelSummary } from '../api/endpoints/catalogue'
import type {
    ConnectedProvider,
    CredentialField,
    CustomProvider,
    EntryPrice,
    EntryPricing,
    LimitsEdited,
    ProfileEntry,
    SsoSessionState,
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

// ---- The saved SSO session (spec 044, FR-009b, FR-034b)
//
// Four states, each with its own label and its own sentence, in one exhaustive record: a fifth
// state the API grows is a compile error here rather than a nameless row. Nothing in these words
// comes from the session itself — the server never sends it, so this module could not print a
// token, a code or a key even if it tried (FR-035).

/** What a row and the form say about a connection's stored session. */
export type SsoStateView = { state: SsoSessionState; profile?: string | null; expires_at?: string | null }

const SSO_STATE_TEXT: Record<SsoSessionState, { label: string; detail: string }> = {
    configured: {
        label: 'Not signed in yet',
        detail: 'The Identity Center connection is saved, but sign in is still needed before this workspace can use it.',
    },
    'signed-in': {
        label: 'Signed in',
        detail: 'A session is stored for this connection and Vizier is using it.',
    },
    expired: {
        label: 'Session expired',
        detail: 'The stored session has run out, so Bedrock calls cannot be made with it. Sign in again to get a new session.',
    },
    unreadable: {
        label: 'Session cannot be read',
        detail: 'The stored session cannot be read with this workspace’s key, so sign in again to replace it.',
    },
}

/** The one label and the one sentence a session state has, shared by the row and the form (rule W5). */
export function ssoStateText(state: SsoSessionState): { label: string; detail: string } {
    return SSO_STATE_TEXT[state]
}

/** The session a connected row carries, or nothing when the API sent no state (the provider does not sign in). */
export function ssoStateOf(p: ConnectedProvider): SsoStateView | undefined {
    if (!p.sso_state) return undefined
    return { state: p.sso_state, profile: p.sso_profile, expires_at: p.session_expires_at }
}

/** What a row says about how it is connected: "Saved key", "OPENAI_API_KEY", an address. */
export function credentialText(p: ConnectedProvider): string {
    const where = p.endpoint ?? p.base_url
    const parts: string[] = []
    // The session's state leads the credential part, so the row reads differently per state and
    // still says nothing the session holds (FR-009b).
    if (p.sso_state) parts.push(ssoStateText(p.sso_state).label)
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

/**
 * What the paired model would fill in for an entry (FR-034a.7). A field a person typed is left out
 * entirely, so a caller cannot overwrite it by accident; every other field is present, and `null`
 * means the model does not state that limit. The form reads this to say what saving will do.
 */
export function limitFill(
    model: ModelSummary,
    limitsEdited?: LimitsEdited | null,
): { context?: number | null; output?: number | null } {
    const fill: { context?: number | null; output?: number | null } = {}
    if (limitsEdited?.context !== true) fill.context = model.limit.context ?? null
    if (limitsEdited?.output !== true) fill.output = model.limit.output ?? null
    return fill
}

/** Where a stored limit came from: a number the person typed, one the model filled, or nothing. */
export type LimitProvenance = 'typed' | 'filled' | 'empty'

/**
 * Where one stored limit came from. `limits_edited` is the server's per-field answer: `false` means it
 * filled the number from the paired model, `true` means a person typed it. An entry saved before the
 * server reported it carries no flag, and every number such an entry has was typed by a person, so an
 * absent flag reads as `typed` — which is what keeps the model from filling over it.
 */
export function limitProvenance(value: number | null | undefined, typed?: boolean | null): LimitProvenance {
    if (value === null || value === undefined || !Number.isFinite(value) || value <= 0) return 'empty'
    return typed === false ? 'filled' : 'typed'
}

/**
 * What one limit field's helper says. These four are the form's whole vocabulary for a limit: a typed
 * number is kept, a filled one says where it came from, an empty one says what saving will fill in —
 * with the number, so nobody has to guess — or that nothing is known (FR-028, FR-034a.7).
 */
export function limitHint(state: LimitProvenance | 'invalid', offered?: number | null, pairedName?: string | null): string {
    if (state === 'invalid') return 'Use a whole number of tokens.'
    if (state === 'typed') return 'Your number — saving keeps it, the paired model does not overwrite it.'
    if (state === 'filled') return `Filled from ${pairedName ?? 'the paired model'} — type to override.`
    if (offered === null || offered === undefined) return 'Leave empty when unknown.'
    return `Filled from ${pairedName ?? 'the paired model'}: ${offered.toLocaleString('en-US')} tokens when you save — type a number to override.`
}

/**
 * An entry's limits as one phrase; an unset one is unknown, never "unsupported" (FR-028). The figures
 * are the stored ones, unchanged; where the server reported their provenance each carries it, so a
 * number the model filled is never read as a hand-entered one.
 */
export function entryLimits(
    entry: Pick<ProfileEntry, 'context' | 'output'> & { limits_edited?: LimitsEdited | null },
): string {
    const limit = (value: number | null | undefined, typed: boolean | undefined, label: string) => {
        if (!value) return `${label} unknown`
        // Without the server's per-field answer there is nothing to claim, so the figure stays bare.
        const marker = !entry.limits_edited
            ? ''
            : limitProvenance(value, typed) === 'filled'
              ? ' (from the paired model)'
              : ' (your number)'
        return `${value.toLocaleString()} ${label}${marker}`
    }
    return [
        limit(entry.context, entry.limits_edited?.context, 'context'),
        limit(entry.output, entry.limits_edited?.output, 'output'),
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
