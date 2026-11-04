import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { EntryRow } from './ModelsDrawer'
import type { EntryPricing, ProfileEntry } from '../../api/endpoints/providers'

const ARN = 'arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123'
const MISSING = 'anthropic.claude-3-5-sonnet-20240620-v1:0'

const NOT_SET: EntryPricing = {
    state: 'not_set',
    flag: 'pricing not set',
    reason: 'no model is paired with this ARN, so there is nothing to price it from — pick one from the list or enter a price by hand',
}
const GONE: EntryPricing = {
    state: 'not_resolved',
    flag: 'pricing not resolved',
    reason: `no price for ${MISSING}: it is no longer in the catalogue — pick another model or enter a price by hand`,
}
const NO_CATALOGUE: EntryPricing = {
    state: 'no_catalogue',
    flag: 'pricing not set',
    reason:
        'the model catalogue carries no Bedrock model list, so there is nothing to price this against yet — this resolves itself once the list is present; or enter a price by hand',
}

function entry(pricing: EntryPricing): ProfileEntry {
    return {
        arn: ARN,
        name: null,
        context: 200000,
        output: 64000,
        display_name: 'abc123',
        pricing,
    }
}

const draw = (pricing: EntryPricing) =>
    renderToString(
        <ul>
            <EntryRow entry={entry(pricing)} />
        </ul>
    )

describe('the inference profile row', () => {
    it('shows the ARN in full and the label with its reason, without a further click', () => {
        const html = draw(NOT_SET)
        expect(html).toContain(ARN)
        expect(html).toContain('abc123')
        expect(html).toContain('200,000 context · 64,000 output')
        expect(html).toContain('pricing not set')
        expect(html).toContain(NOT_SET.reason)
    })

    it('never prints a bare label: each absence carries its own non-empty reason', () => {
        for (const pricing of [NOT_SET, GONE, NO_CATALOGUE]) {
            const html = draw(pricing)
            expect(pricing.reason.trim().length).toBeGreaterThan(pricing.flag.length)
            expect(html).toContain(pricing.flag)
            expect(html).toContain(pricing.reason)
        }
    })

    it('keeps the three causes distinct, so no two rows share wording', () => {
        const reasons = [NOT_SET, GONE, NO_CATALOGUE].map((pricing) => pricing.reason)
        expect(new Set(reasons).size).toBe(3)
        for (const pricing of [NOT_SET, GONE, NO_CATALOGUE]) {
            const html = draw(pricing)
            for (const other of [NOT_SET, GONE, NO_CATALOGUE].filter((p) => p !== pricing)) {
                expect(html).not.toContain(other.reason)
            }
        }
        // The gone cause names the missing id inside the reason; the other two do not.
        expect(draw(GONE)).toContain(MISSING)
        expect(draw(NOT_SET)).not.toContain(MISSING)
        expect(draw(NO_CATALOGUE)).not.toContain(MISSING)
    })

    it('shows the figure and no absence wording once the entry is priced', () => {
        const html = draw({ state: 'from_catalogue', cost: { input: 3, output: 15 }, model: MISSING })
        // React separates adjacent text nodes in SSR, so the figures are checked one by one.
        expect(html).toContain('$3.00')
        expect(html).toContain('$15.00')
        expect(html).toContain('per 1M')
        expect(html).toContain(`from ${MISSING}`)
        expect(html).not.toContain('pricing not set')
        expect(html).not.toContain('pricing not resolved')
    })
})
