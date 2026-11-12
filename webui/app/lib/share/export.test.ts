import { describe, expect, it } from 'vitest'
import { escapeHtml } from './embed'
import { buildOfflineHtml } from './export'
import type { Snapshot } from './schema'
import { exportFileName } from './slug'

describe('exportFileName', () => {
    it('creates clean filenames bounded to 60 characters with date stamp', () => {
        const date = new Date(2026, 10, 8) // 8 Nov 2026 -> 20261108
        expect(exportFileName('Trip planning with Atlas', date)).toBe(
            'trip-planning-with-atlas-20261108.vizier.html'
        )

        // Windows reserved name
        expect(exportFileName('CON', date)).toBe('conversation-20261108.vizier.html')
        expect(exportFileName('aux', date)).toBe('conversation-20261108.vizier.html')

        // Empty or non-alphanumeric title
        expect(exportFileName('', date)).toBe('conversation-20261108.vizier.html')
        expect(exportFileName('???///:::***', date)).toBe('conversation-20261108.vizier.html')

        // Long title clamped to <= 60 chars before date
        const longTitle = 'a'.repeat(100)
        const name = exportFileName(longTitle, date)
        expect(name).toBe(`${'a'.repeat(60)}-20261108.vizier.html`)
    })
})

describe('buildOfflineHtml', () => {
    const template = `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>Vizier</title>
</head>
<body>
<div id="root"></div>
<!-- VIZIER_SNAPSHOT_MARKER -->
<script type="module" src="/app/viewer/main.tsx"></script>
</body>
</html>`

    const snapshot: Snapshot = {
        format_version: 1,
        generator: 'test',
        created_at: 1790000000000,
        title: 'Trip to <Kyoto> & "Osaka"',
        source: { agent_name: 'Atlas' },
        include_trail: false,
        turns: [
            {
                id: 't1',
                role: 'person',
                at: 1,
                parts: [{ kind: 'text', markdown: 'Hello world' }],
            },
        ],
        media: {},
        rehydration: null,
    }

    it('substitutes marker with escaped snapshot and escapes title', () => {
        const html = buildOfflineHtml(template, snapshot)

        expect(html).toContain('<title>Trip to &lt;Kyoto&gt; &amp; &quot;Osaka&quot; · Vizier</title>')
        expect(html).toContain('<script type="application/json" id="vizier-snapshot">')
        expect(html).not.toContain('<!-- VIZIER_SNAPSHOT_MARKER -->')

        // No external link, import, or base tags
        expect(html).not.toContain('<link')
        expect(html).not.toContain('@import')
        expect(html).not.toContain('<base')
        expect(html).not.toContain('<form')
    })
})

describe('buildOfflineHtml: replacement patterns in content are literal', () => {
    const template = '<html><head><title>Vizier</title></head><body><div id="root"></div><!-- VIZIER_SNAPSHOT_MARKER --></body></html>'
    const base: Snapshot = {
        format_version: 1,
        generator: 'test',
        created_at: 1,
        title: 'T',
        source: { agent_name: 'Atlas' },
        include_trail: false,
        turns: [],
        media: {},
        rehydration: null,
    }
    const tagOf = (html: string) => {
        const m = /<script type="application\/json" id="vizier-snapshot">([\s\S]*?)<\/script>/.exec(html)
        expect(m).not.toBeNull()
        return JSON.parse(m![1]) as Snapshot
    }

    for (const pattern of ['$&', "$'", '$`', '$$', '$1', '$<name>']) {
        it(`keeps ${pattern} in a turn and in the title unchanged`, () => {
            const text = `price ${pattern} and ${pattern}${pattern} end`
            const snap: Snapshot = {
                ...base,
                title: `Cost ${pattern}`,
                turns: [{ id: 't1', role: 'person', at: 1, parts: [{ kind: 'text', markdown: text }] }],
            }
            const html = buildOfflineHtml(template, snap)
            const back = tagOf(html)
            expect(back.turns[0].parts[0]).toEqual({ kind: 'text', markdown: text })
            expect(back.title).toBe(`Cost ${pattern}`)
            expect(html).toContain(`<title>${escapeHtml(`Cost ${pattern}`)} · Vizier</title>`)
            // The template around the marker is intact and nothing was duplicated.
            expect(html.startsWith('<html><head>')).toBe(true)
            expect(html.endsWith('</script></body></html>')).toBe(true)
            expect(html.split('id="vizier-snapshot"').length).toBe(2)
        })

        it(`keeps ${pattern} when the template has no marker (inserted before </body>)`, () => {
            const snap: Snapshot = { ...base, turns: [{ id: 't', role: 'person', at: 1, parts: [{ kind: 'text', markdown: pattern }] }] }
            const html = buildOfflineHtml('<html><title>x</title><body></body></html>', snap)
            expect(tagOf(html).turns[0].parts[0]).toEqual({ kind: 'text', markdown: pattern })
            expect(html.split('id="vizier-snapshot"').length).toBe(2)
        })
    }
})

describe('exportFileName: reserved names and separators', () => {
    const date = new Date(2026, 10, 8)
    const tail = '-20261108.vizier.html'
    it.each(['CON', 'prn', 'Aux', 'NUL', 'com1', 'COM9', 'lpt1', 'LPT9', ' con ', 'con.', '..con..'])('replaces reserved name %j', (t) => {
        expect(exportFileName(t, date)).toBe(`conversation${tail}`)
    })
    it('does not treat names that merely start with a reserved word as reserved', () => {
        expect(exportFileName('console', date)).toBe(`console${tail}`)
        expect(exportFileName('com10', date)).toBe(`com10${tail}`)
    })
    it.each(['a/b', 'a\\b', 'a:b', 'a*b', 'a?b', 'a"b', 'a<b>c', 'a|b', '../../etc/passwd', 'C:\\Windows\\x'])('never lets a separator or traversal out of %j', (t) => {
        const name = exportFileName(t, date)
        expect(name).toMatch(/^[a-z0-9]+(?:-[a-z0-9]+)*-20261108\.vizier\.html$/)
        expect(name).not.toMatch(/[\\/:*?"<>|]|\.\./)
    })
    it('drops control characters, NUL, bidi marks and a trailing dot', () => {
        expect(exportFileName('a\u0000b\u202Ec\n d.', date)).toBe(`a-b-c-d${tail}`)
    })
})
