import { describe, expect, it } from 'vitest'
import type { ModelSummary } from '../api/endpoints/catalogue'
import { capabilityLine, inputPhrase } from './models'

const model = (patch: Partial<ModelSummary>): ModelSummary => ({
    id: 'm',
    name: 'M',
    default: false,
    limit: {},
    caps: {},
    status: 'active',
    ...patch,
})

describe('capabilityLine', () => {
    it('names input, context and price', () => {
        const line = capabilityLine(
            model({ caps: { image_input: true, tool_call: true }, limit: { context: 200_000 }, cost: { input: 3, output: 15 } })
        )
        expect(line).toBe('Text, image and tool input · 200k context · $3 / $15 per M tokens')
    })

    it('leaves out what the catalogue does not know', () => {
        expect(capabilityLine(model({}))).toBe('Text input')
    })

    it('shows sub-dollar prices with their cents', () => {
        expect(capabilityLine(model({ cost: { input: 0.25, output: 1.25 } }))).toBe('Text input · $0.25 / $1.25 per M tokens')
    })
})

describe('inputPhrase', () => {
    it('joins two kinds with "and"', () => {
        expect(inputPhrase({ tool_call: true })).toBe('Text and tool input')
    })
})
