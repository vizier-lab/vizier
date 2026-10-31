import { describe, expect, it } from 'vitest'
import { parseUnifiedDiff } from './diff'

describe('parseUnifiedDiff', () => {
    it('keeps hunks, added, removed and context lines, numbered, and drops the file headers', () => {
        const text = [
            'diff --git a/a.rs b/a.rs',
            'index 123..456 100644',
            '--- a/a.rs',
            '+++ b/a.rs',
            '@@ -1,2 +1,2 @@',
            ' fn main() {',
            '-    greet();',
            '+    hello();',
        ].join('\n')
        expect(parseUnifiedDiff(text)).toEqual([
            { kind: 'hunk', text: '@@ -1,2 +1,2 @@' },
            { kind: 'ctx', text: 'fn main() {', oldNo: 1, newNo: 1 },
            { kind: 'del', text: '    greet();', oldNo: 2 },
            { kind: 'add', text: '    hello();', newNo: 2 },
        ])
    })

    it('restarts the numbers at each hunk', () => {
        const text = ['@@ -1,1 +1,1 @@', ' a', '@@ -10,1 +20,1 @@', ' b'].join('\n')
        const ctx = parseUnifiedDiff(text).filter((l) => l.kind === 'ctx')
        expect(ctx.map((l) => [l.oldNo, l.newNo])).toEqual([
            [1, 1],
            [10, 20],
        ])
    })

    it('an empty diff has no lines', () => {
        expect(parseUnifiedDiff('')).toEqual([])
    })
})
