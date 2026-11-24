import { expect, it } from 'vitest'
import { authorLabel, diffLines, type RevisionDiff } from './revisions'

it('names who made a save', () => {
    const user = { type: 'user', user_id: 'u', username: 'ganesha' } as const
    expect(authorLabel(user, { type: 'webui' })).toBe('ganesha · WebUI')
    expect(authorLabel({ type: 'agent' }, { type: 'dream' })).toBe('dream cycle')
    expect(authorLabel({ type: 'agent' }, { type: 'conversation' }, 'Archivist')).toBe('Archivist · in a conversation')
    expect(authorLabel(user, { type: 'rollback', restored_from: 3 })).toBe('restored from rev 3')
    expect(authorLabel({ type: 'system' }, { type: 'baseline' })).toBe('first version')
})

it('maps a diff to the library lines, a header per hunk and the numbers kept', () => {
    const diff: RevisionDiff = {
        from_seq: 1, to_seq: 2, additions: 1, deletions: 1,
        hunks: [
            { old_start: 1, old_lines: 2, new_start: 1, new_lines: 2, lines: [
                { op: 'equal', old_line: 1, new_line: 1, text: 'a' },
                { op: 'delete', old_line: 2, new_line: null, text: 'b' },
            ] },
            { old_start: 9, old_lines: 1, new_start: 9, new_lines: 1, lines: [{ op: 'insert', old_line: null, new_line: 9, text: 'c' }] },
        ],
    }
    expect(diffLines(diff).map((l) => l.kind)).toEqual(['hunk', 'ctx', 'del', 'hunk', 'add'])
    expect(diffLines(diff)[2]).toMatchObject({ oldNo: 2, text: 'b' })
})
