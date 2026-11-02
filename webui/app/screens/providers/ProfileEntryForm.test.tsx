import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { ProfileEntryForm, type ProfileEntry } from './ProfileEntryForm'

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
