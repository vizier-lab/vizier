import { describe, expect, it } from 'vitest'
import type { ModelSummary } from '../api/endpoints/catalogue'
import type { ConnectedProvider, EntryPricing, SsoSessionState } from '../api/endpoints/providers'
import { credentialText, draftPrice, entryLimit, pricingCandidates, pricingNote, ssoStateOf, ssoStateText } from './providers'

const base: ConnectedProvider = {
    id: 'amazon-bedrock',
    name: 'Amazon Bedrock',
    origin: 'catalogue',
    kind: 'cloud',
    model_count: 200,
    fields: ['access_key_id', 'secret_access_key', 'session_token', 'region'],
    source: 'saved',
    has_api_key: false,
    has_access_token: false,
    used_by: { chat: 0, stt: 0, tts: 0, image_gen: 0, embedding: 0 },
}

describe('the Bedrock connection summary', () => {
    it('shows the saved key pair and the region, never a secret', () => {
        const text = credentialText({
            ...base,
            access_key_id: 'AKIAEXAMPLE',
            has_secret_access_key: true,
            has_session_token: false,
            region: 'us-east-1',
        })
        expect(text).toBe('Saved keys · us-east-1')
        expect(text).not.toContain('AKIAEXAMPLE')
    })

    it('names the environment variable that supplied the keys', () => {
        expect(
            credentialText({
                ...base,
                source: 'env:AWS_ACCESS_KEY_ID',
                access_key_id: 'AKIAENV',
                has_secret_access_key: true,
                region: 'eu-west-1',
            })
        ).toBe('Keys from AWS_ACCESS_KEY_ID · eu-west-1')
    })

    it('says a half-configured connection has no secret key', () => {
        expect(
            credentialText({ ...base, source: null, access_key_id: 'AKIAHALF', has_secret_access_key: false, region: 'us-east-1' })
        ).toBe('no secret key · us-east-1')
    })
})

const MISSING = 'anthropic.claude-3-5-sonnet-20240620-v1:0'

// The three absences SC-021 enumerates, carrying the reasons the server derives for them: this
// module only renders them, so the fixture is what the wire carries (contracts §4a).
const NOT_SET: EntryPricing = {
    state: 'not_set',
    flag: 'pricing not set',
    reason: 'no model is paired with this ARN, so there is nothing to price it from — pick one from the list or enter a price by hand',
}
const GONE: EntryPricing = {
    state: 'not_resolved',
    flag: 'pricing not resolved',
    model: MISSING,
    reason: `no price for ${MISSING}: it is no longer in the catalogue — pick another model or enter a price by hand`,
}
const NO_CATALOGUE: EntryPricing = {
    state: 'no_catalogue',
    flag: 'pricing not set',
    reason:
        'the model catalogue carries no Bedrock model list, so there is nothing to price this against yet — this resolves itself once the list is present; or enter a price by hand',
}

describe('the inference profile price note', () => {
    it('carries a non-empty reason for every unpriced cause, never a bare label', () => {
        for (const pricing of [NOT_SET, GONE, NO_CATALOGUE]) {
            const note = pricingNote(pricing)
            expect(note.priced).toBe(false)
            if (note.priced) continue
            expect(note.reason.trim().length).toBeGreaterThan(0)
            // The reason says more than the label it stands under, so the label is never the answer.
            expect(note.reason.length).toBeGreaterThan(note.flag.length)
            expect(note.flag).toBe(pricing.flag)
        }
    })

    it('keeps the causes distinct from the reason alone, not from the shared label', () => {
        const reasons = [NOT_SET, GONE, NO_CATALOGUE].flatMap((pricing) => {
            const note = pricingNote(pricing)
            return note.priced ? [] : [note.reason]
        })
        expect(new Set(reasons).size).toBe(3)
        // `pricing not set` stands over two causes, so the label cannot be what tells them apart.
        const labels = [NOT_SET, GONE, NO_CATALOGUE].flatMap((pricing) => {
            const note = pricingNote(pricing)
            return note.priced ? [] : [note.flag]
        })
        expect(new Set(labels).size).toBe(2)
        // The gone cause names the missing model inside its reason.
        expect(GONE.reason).toContain(MISSING)
        expect(NOT_SET.reason).not.toContain(MISSING)
        expect(NO_CATALOGUE.reason).not.toContain(MISSING)
    })

    it('renders no absence wording once the entry is priced', () => {
        const fromCatalogue = pricingNote({ state: 'from_catalogue', cost: { input: 3, output: 15 }, model: MISSING })
        expect(fromCatalogue).toEqual({ priced: true, text: `$3.00 in / $15.00 out per 1M · from ${MISSING}` })
        expect('reason' in fromCatalogue).toBe(false)
        expect('flag' in fromCatalogue).toBe(false)

        const explicit = pricingNote({ state: 'explicit', cost: { input: 0.25, output: 1 }, source: 'entered price' })
        expect(explicit).toEqual({ priced: true, text: '$0.250 in / $1.00 out per 1M · entered price' })
        expect('reason' in explicit).toBe(false)
    })
})

describe('the entry form helpers', () => {
    it('takes a limit only as a positive whole number', () => {
        expect(entryLimit(' 200000 ')).toBe(200000)
        expect(entryLimit('')).toBeNull()
        expect(Number.isNaN(entryLimit('-1'))).toBe(true)
        expect(Number.isNaN(entryLimit('1.5'))).toBe(true)
    })

    it('sends a price only when both rates are usable, keeping the cached rates it was given', () => {
        expect(draftPrice('', '')).toEqual({ price: null, touched: false, invalid: false })
        expect(draftPrice('3', '')).toEqual({ price: null, touched: true, invalid: true })
        expect(draftPrice('3', 'fifteen')).toEqual({ price: null, touched: true, invalid: true })
        expect(draftPrice('3', '15', { input: 1, output: 1, cache_read: 0.3, cache_write: 1.5 })).toEqual({
            price: { input: 3, output: 15, cache_read: 0.3, cache_write: 1.5 },
            touched: true,
            invalid: false,
        })
    })

    it('offers only catalogue rows as pricing candidates, never the saved ARNs', () => {
        const row = (id: string, origin: string): ModelSummary => ({ id, name: id, default: false, limit: {}, caps: {}, status: 'active', origin })
        const rows = pricingCandidates([
            row('amazon.nova-lite-v1:0', 'catalogue'),
            row('arn:aws:bedrock:us-east-1:123456789012:inference-profile/x', 'inference_profile'),
        ])
        expect(rows.map((m) => m.id)).toEqual(['amazon.nova-lite-v1:0'])
    })
})

// ---- The saved session's four states (spec 044, FR-009b, FR-034b)

const STATES: SsoSessionState[] = ['configured', 'signed-in', 'expired', 'unreadable']

/** The SSO connection as the list carries it: non-secret fields, a region, no key pair. */
const SSO_ROW: ConnectedProvider = {
    ...base,
    id: 'amazon-bedrock-sso',
    name: 'Amazon Bedrock (IAM Identity Center)',
    fields: ['start_url', 'sso_region', 'role_name', 'sso_profile', 'region'],
    region: 'us-east-1',
}

describe('the saved session state', () => {
    it('gives every state a label and a sentence of its own', () => {
        const texts = STATES.map((state) => ssoStateText(state))
        expect(new Set(texts.map((t) => t.label)).size).toBe(STATES.length)
        expect(new Set(texts.map((t) => t.detail)).size).toBe(STATES.length)
        // The sentence is what answers the row, so it says more than the label over it.
        for (const t of texts) expect(t.detail.length).toBeGreaterThan(t.label.length)
    })

    it('says how to get out of an expired or unreadable session, not just that it is broken', () => {
        expect(ssoStateText('expired').label).toBe('Session expired')
        expect(ssoStateText('expired').detail).toContain('Sign in again')
        // Unreadable names its cause, which is the reason it cannot be fixed in place.
        expect(ssoStateText('unreadable').label).toBe('Session cannot be read')
        expect(ssoStateText('unreadable').detail).toContain('cannot be read with this workspace')
        expect(ssoStateText('unreadable').detail).toContain('sign in again')
        // Configured is its own thing: saved, and still waiting for a sign-in.
        expect(ssoStateText('configured').label).toBe('Not signed in yet')
        expect(ssoStateText('configured').detail).toContain('connection is saved')
        expect(ssoStateText('configured').detail).toContain('sign in is still needed')
    })

    it('carries the profile and the expiry of the row, and nothing when there is no state', () => {
        expect(ssoStateOf(base)).toBeUndefined()
        expect(ssoStateOf({ ...SSO_ROW, sso_state: null })).toBeUndefined()
        expect(
            ssoStateOf({ ...SSO_ROW, sso_state: 'signed-in', sso_profile: 'team-dev', session_expires_at: '2026-11-03T13:12:00Z' })
        ).toEqual({ state: 'signed-in', profile: 'team-dev', expires_at: '2026-11-03T13:12:00Z' })
    })

    it('leads the row with the state, keeps the region, and still names no secret', () => {
        const text = credentialText({ ...SSO_ROW, sso_state: 'expired' })
        expect(text.startsWith('Session expired')).toBe(true)
        expect(text).toContain('us-east-1')
        // A row without a session state reads exactly as it did before (FR-034b).
        expect(credentialText({ ...SSO_ROW, sso_state: undefined })).toBe('us-east-1')
    })
})
