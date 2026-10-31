import { describe, expect, it } from 'vitest'
import { parseUnifiedDiff } from './diff'

describe('parseUnifiedDiff', () => {
    it('keeps added, removed and context lines and drops the headers', () => {
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
            { kind: 'ctx', text: 'fn main() {' },
            { kind: 'del', text: '    greet();' },
            { kind: 'add', text: '    hello();' },
        ])
    })

    it('an empty diff has no lines', () => {
        expect(parseUnifiedDiff('')).toEqual([])
    })
})
