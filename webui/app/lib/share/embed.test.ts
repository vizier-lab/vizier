import fs from 'node:fs'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { embedJson, escapeHtml } from './embed'

describe('embedJson', () => {
    it('escapes breakout characters so that HTML parsers cannot treat them as tags', () => {
        const input = {
            danger: '</script><script>alert(1)</script><!--<script>',
            separators: 'line\u2028sep\u2029para',
            amp: 'foo & bar',
            angles: '< > &',
        }
        const embedded = embedJson(input)

        expect(embedded).not.toContain('</script')
        expect(embedded).not.toContain('<script')
        expect(embedded).not.toContain('<!--')
        expect(embedded).not.toContain('<')
        expect(embedded).not.toContain('>')
        expect(embedded).not.toContain('&')
        expect(embedded).not.toContain('\u2028')
        expect(embedded).not.toContain('\u2029')

        // JSON.parse on the embedded string reconstructs the exact original value
        const parsed = JSON.parse(embedded)
        expect(parsed).toEqual(input)
    })

    it('round-trips all hostile fixtures without breakout or data loss', () => {
        const dir = path.join(__dirname, 'fixtures/hostile')
        const files = fs.readdirSync(dir).filter((f) => f.endsWith('.json'))

        for (const file of files) {
            const raw = fs.readFileSync(path.join(dir, file), 'utf8')
            const obj = JSON.parse(raw)
            const embedded = embedJson(obj)

            expect(embedded).not.toContain('</script')
            expect(embedded).not.toContain('<!--')
            expect(embedded).not.toContain('<')
            expect(embedded).not.toContain('>')
            expect(embedded).not.toContain('&')
            expect(embedded).not.toContain('\u2028')
            expect(embedded).not.toContain('\u2029')

            const recovered = JSON.parse(embedded)
            expect(recovered).toEqual(obj)
        }
    })

    it('round-trips a large string with unicode', () => {
        const chunks: string[] = []
        for (let i = 0; i < 50_000; i++) {
            chunks.push(`test <script> & <!-- ${i} \u2028 \u2029`)
        }
        const large = { data: chunks.join(' ') }
        const embedded = embedJson(large)

        expect(embedded).not.toContain('</script')
        expect(embedded).not.toContain('<')
        expect(embedded).not.toContain('>')
        expect(embedded).not.toContain('&')

        const parsed = JSON.parse(embedded)
        expect(parsed).toEqual(large)
    })
})

describe('escapeHtml', () => {
    it('escapes special html chars', () => {
        expect(escapeHtml('<script>"hello" & \'world\'</script>')).toBe(
            '&lt;script&gt;&quot;hello&quot; &amp; &#39;world&#39;&lt;/script&gt;'
        )
    })
})
