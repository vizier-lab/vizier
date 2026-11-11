import { describe, expect, it } from 'vitest'
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
