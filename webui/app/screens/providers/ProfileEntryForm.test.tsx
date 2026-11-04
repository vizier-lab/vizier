import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import type { ModelSummary } from '../../api/endpoints/catalogue'
import type { ProfileEntry } from '../../api/endpoints/providers'
import { limitFill, limitProvenance } from '../../lib/providers'
import { ProfileEntryForm } from './ProfileEntryForm'

const ARN = 'arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123'

function entry(over: Partial<ProfileEntry> = {}): ProfileEntry {
    return {
        arn: ARN,
        name: 'Acme Prod — Claude',
        context: 200000,
        output: 64000,
        model: null,
        price: null,
        display_name: 'Acme Prod — Claude',
        pricing: { state: 'not_set', flag: 'pricing not set', reason: 'no model is paired with this ARN' },
        ...over,
    }
}

const draw = (e?: ProfileEntry) =>
    renderToString(<ProfileEntryForm providerId="amazon-bedrock-sso" entry={e} onSaved={() => {}} onCancel={() => {}} />)

describe('the inference profile form', () => {
    it('asks for the ARN, the optional name and limits, the catalogue model and the price', () => {
        const html = draw()
        expect(html).toContain('Add an inference profile')
        for (const label of [
            'ARN',
            'Name (optional)',
            'Context window (optional)',
            'Max output (optional)',
            'Model the usage is priced from',
            'Or set a price by hand (USD per 1M tokens)',
        ]) {
            expect(html).toContain(label)
        }
        // The catalogue picker is the design system's, closed in its trigger state.
        expect(html).toContain('Pricing model: none chosen')
        expect(html).toContain('No model — price by hand or leave unpriced')
        expect(html).toContain('Vizier sends it unchanged')
        expect(html).toContain('Add entry')
    })

    it('offers both limits of the paired model and never fills over a typed one', () => {
        const model: ModelSummary = {
            id: 'amazon.nova-lite-v1:0',
            name: 'Nova Lite',
            default: false,
            limit: { context: 200000, output: 64000 },
            caps: {},
            status: 'active',
        }

        // Nothing typed: the model offers both numbers, which is what the helper sentence promises.
        expect(limitFill(model)).toEqual({ context: 200000, output: 64000 })

        // The person typed the context window, so the model does not offer it — the number is kept —
        // while the output limit it did fill is still offered (FR-034a.7).
        const kept = limitFill(model, { context: true, output: false })
        expect(kept.context).toBeUndefined()
        expect(kept.output).toBe(64000)

        // Provenance per field: the server's flag wins; a number from before the flag existed was
        // typed by a person, so it reads as theirs and is never filled over.
        expect(limitProvenance(200000, true)).toBe('typed')
        expect(limitProvenance(200000, false)).toBe('filled')
        expect(limitProvenance(200000, undefined)).toBe('typed')
        expect(limitProvenance(null, true)).toBe('empty')
    })

    it('says the paired model fills the limits still empty, and no longer denies it', () => {
        const html = draw()
        expect(html).not.toContain('It never supplies the limits above')
        expect(html).toContain('it fills the limits that are still empty when you save')
        expect(html).toContain('A number you type wins')
        // With no model chosen there is nothing to fill from, and the field still says what empty means.
        expect(html).toContain('Leave empty when unknown')
    })

    it('marks a typed limit as kept and a model-filled one as the model’s', () => {
        const html = draw(entry({ model: 'amazon.nova-lite-v1:0', limits_edited: { context: true, output: false } }))
        expect(html).toContain('Your number — saving keeps it, the paired model does not overwrite it.')
        expect(html).toContain('Filled from amazon.nova-lite-v1:0 — type to override.')
    })

    it('opens a saved entry with its ARN locked and its stored price and pairing shown', () => {
        const html = draw(entry({ price: { input: 2, output: 8 }, model: 'amazon.nova-lite-v1:0' }))
        expect(html).toContain(ARN)
        // The ARN identifies the entry, so it is read-only here — a new ARN is a new entry.
        expect(html).toContain('readOnly=""')
        expect(html).toContain('value="2"')
        expect(html).toContain('value="8"')
        expect(html).toContain('Pricing model: amazon.nova-lite-v1:0')
        expect(html).toContain('Wins over the catalogue model')
        expect(html).toContain('Save entry')
    })
})
