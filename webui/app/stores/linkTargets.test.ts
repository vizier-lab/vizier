import { describe, expect, it } from 'vitest'
import type { LinkTargets } from '../api/endpoints/linkTargets'
import { indexHas, suggestionsFor } from './linkTargets'

const index: LinkTargets = {
    artifacts: ['report', 'report-final'],
    functions: ['fetch_weather'],
    tasks: [{ slug: 'weekly-report', title: 'Weekly report' }],
    skills: [
        { slug: 'summarize', scope: 'agent' },
        { slug: 'onboarding', scope: 'global' },
    ],
    memory: { bundles: ['research'], docs: [{ bundle: 'research', path: 'notes' }] },
    truncated: { artifacts: true },
}

describe('suggestionsFor', () => {
    it('lists every kind with its note, memory docs as bundle/path', () => {
        const rows = suggestionsFor(index, null, '')
        expect(rows.map((r) => r.kind)).toEqual([
            'artifact',
            'artifact',
            'function',
            'task',
            'skill',
            'skill',
            'memory',
            'memory',
        ])
        expect(rows.find((r) => r.name === 'summarize')?.note).toBe('skill · agent')
        expect(rows.find((r) => r.name === 'research/notes')?.note).toBe('memory')
    })

    it('narrows to one kind after the colon, then by name', () => {
        expect(suggestionsFor(index, 'skill', '').map((r) => r.name)).toEqual(['summarize', 'onboarding'])
        expect(suggestionsFor(index, 'task', 'week').map((r) => r.name)).toEqual(['weekly-report'])
        expect(suggestionsFor(index, 'artifact', 'zzz')).toEqual([])
    })

    it('without a colon matches names and the kind word itself', () => {
        expect(suggestionsFor(index, null, 'report').map((r) => r.name)).toEqual([
            'report',
            'report-final',
            'weekly-report',
        ])
        expect(suggestionsFor(index, null, 'sk').map((r) => r.kind)).toEqual(['skill', 'skill'])
        expect(suggestionsFor(index, null, 'zzz')).toEqual([])
    })
})

describe('indexHas', () => {
    it('answers presence for every kind, the memory doc form, and nothing else', () => {
        expect(indexHas(index, 'artifact', 'report')).toBe(true)
        expect(indexHas(index, 'artifact', 'gone')).toBe(false)
        expect(indexHas(index, 'function', 'fetch_weather')).toBe(true)
        expect(indexHas(index, 'task', 'weekly-report')).toBe(true)
        expect(indexHas(index, 'skill', 'onboarding')).toBe(true)
        expect(indexHas(index, 'memory', 'research')).toBe(true)
        expect(indexHas(index, 'memory', 'research/notes')).toBe(true)
        expect(indexHas(index, 'memory', 'research/gone')).toBe(false)
        expect(indexHas(index, 'flavour', 'x')).toBe(false)
    })
})
