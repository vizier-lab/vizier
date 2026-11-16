import { describe, expect, it } from 'vitest'
import { ApiError } from '../api/client'
import { authorLabel, currentVersionOf, isOutOfDate, missingCalls, nameTaken, offNote, parseSchema, problemText, problemsOf } from './functions'

describe('functions', () => {
    it('names who made a version', () => {
        expect(authorLabel({ kind: 'agent' }, 'Archivist')).toBe('Archivist')
        expect(authorLabel({ kind: 'dream', dream_cycle_id: 'c1' })).toBe('a dream')
        expect(authorLabel({ kind: 'person', name: 'alice' })).toBe('alice')
        expect(offNote({ kind: 'person', name: 'alice' })).toBe('switched off by alice')
        expect(offNote(null)).toBe('switched off')
    })

    it('reads a stale edit and its current version', () => {
        const stale = new ApiError(409, 'out_of_date', 'changed', { status: 'out_of_date', current_version: 4 })
        expect(isOutOfDate(stale)).toBe(true)
        expect(currentVersionOf(stale)).toBe(4)
        expect(isOutOfDate(new ApiError(409, 'name_taken', 'taken'))).toBe(false)
    })

    it('lists every problem of a refusal, with position and nearest names', () => {
        const refused = new ApiError(422, 'invalid', 'x', {
            problems: [
                { code: 'unknown_tool', message: 'memory_serch is not a tool you have', did_you_mean: ['memory_search'] },
                { code: 'script_syntax', message: "expected ')'", line: 2, column: 40 },
            ],
        })
        expect(problemsOf(refused).map(problemText)).toEqual([
            'memory_serch is not a tool you have Did you mean memory_search?',
            "line 2, column 40: expected ')'",
        ])
        expect(problemsOf(new Error('boom'))).toEqual([{ code: 'error', message: 'boom' }])
    })

    it('reads missing calls and the schema text', () => {
        expect(missingCalls({ missing: ['stripe_list_charges'] })).toEqual(['stripe_list_charges'])
        expect(missingCalls(null)).toEqual([])
        expect(nameTaken({ missing: [], name_taken: true })).toBe(true)
        expect(nameTaken({ missing: ['x'] })).toBe(false)
        expect(parseSchema('{"type":"object"}')).toEqual({ ok: true, value: { type: 'object' } })
        expect(parseSchema('{').ok).toBe(false)
    })
})
