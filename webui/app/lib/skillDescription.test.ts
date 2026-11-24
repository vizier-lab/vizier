import { describe, expect, it } from 'vitest'
import { skillDescription } from './skillDescription'

describe('a skill description in a menu row', () => {
    it('keeps a short one as written', () => {
        expect(skillDescription('Stop what the agent is doing')).toBe('Stop what the agent is doing')
    })

    it('takes the first sentence of several', () => {
        expect(skillDescription('Review a diff for bugs. Then review it for clarity.')).toBe(
            'Review a diff for bugs.'
        )
    })

    it('does not end a sentence at an abbreviation', () => {
        expect(skillDescription('Check e.g. the router. Then the store.')).toBe(
            'Check e.g. the router.'
        )
    })

    it('cuts a long sentence at a word, within the limit', () => {
        const text =
            'design, redesign, shape, critique, audit, polish, simplify, harden, animate or otherwise improve a frontend interface: websites, landing pages, dashboards'
        const description = skillDescription(text)
        expect(description.length).toBeLessThanOrEqual(111)
        // Whole words of the original, never a word cut in half.
        expect(description).toBe(
            'design, redesign, shape, critique, audit, polish, simplify, harden, animate or otherwise improve a frontend…'
        )
        expect(text.startsWith(description.slice(0, -1))).toBe(true)
    })

    it('never leaves a separator before the ellipsis', () => {
        expect(skillDescription('one two three four five, six seven eight nine ten', 20)).toBe(
            'one two three four…'
        )
    })

    it('flattens a wrapped paragraph', () => {
        expect(skillDescription('Take over maintenance of an\nunfamiliar repository.')).toBe(
            'Take over maintenance of an unfamiliar repository.'
        )
    })

    it('draws nothing for an empty description', () => {
        expect(skillDescription('')).toBe('')
        expect(skillDescription('   \n ')).toBe('')
    })
})
