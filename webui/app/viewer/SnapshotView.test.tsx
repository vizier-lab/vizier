// T025: the viewer draws a shared copy and nothing else. Hostile text stays text.
import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { RECIPES } from '../lib/generativeUi/recipes/index'
import { createViewerHandlers, NOT_IN_A_COPY } from '../lib/share/actions'
import { parseSnapshot, type Snapshot } from '../lib/share/schema'
import { loadConversation } from '../lib/share/testing'
import { mediaFiles, mediaSrc } from './media'
import { READ_ONLY_LINE, SnapshotView } from './SnapshotView'
import { createStateStore } from '@json-render/core'

const PNG = loadConversation().pngBase64
const imageSpec = { root: 'i', elements: { i: { type: 'Image', props: { file: 'm1', alt: 'from the map' } } } }

function snap(overrides: Record<string, unknown> = {}, parts: unknown[] = []): Snapshot {
    const r = parseSnapshot({
        format_version: 1,
        created_at: Date.UTC(2026, 10, 8),
        title: 'Trip planning',
        source: { agent_name: 'Atlas' },
        include_trail: false,
        turns: [
            { id: 't1', role: 'person', at: 1, parts: [{ kind: 'text', markdown: 'Plan a **3-day** trip' }] },
            {
                id: 't2',
                role: 'agent',
                at: 2,
                parts: [
                    { kind: 'text', markdown: '## Plan\n\n| a | b |\n|---|---|\n| 1 | 2 |' },
                    { kind: 'media', key: 'm1', alt: 'a pixel' },
                    { kind: 'ui', title: 'Image page', spec: imageSpec },
                    { kind: 'ui', title: 'Terrain', spec: RECIPES.scene_3d.spec },
                    { kind: 'placeholder', reason: 'oversized', label: 'An image too large to include' },
                    ...parts,
                ],
            },
        ],
        media: { m1: { mime: 'image/png', bytes: 70, data: PNG } },
        rehydration: null,
        ...overrides,
    })
    if (!r.ok) throw new Error(r.message)
    return r.snapshot
}
const draw = (s: Snapshot) => renderToString(<SnapshotView snapshot={s} />).replace(/<!-- -->/g, '')

describe('SnapshotView', () => {
    const html = draw(snap())

    it('shows the header, turns, and the read-only footer', () => {
        expect(html).toContain('Trip planning')
        expect(html).toContain('Shared snapshot')
        expect(html).toContain('activity trail not included')
        expect(html).toContain('Atlas')
        expect(html).toContain('<strong>3-day</strong>')
        expect(html).toContain('<table')
        expect(html).toContain(READ_ONLY_LINE.replace("'", '&#x27;'))
    })

    it('draws an image from the media map, a ui part, a Scene3D and a placeholder', () => {
        expect(html).toContain(`src="data:image/png;base64,${PNG}"`)
        expect(html).toContain('alt="a pixel"')
        expect(html).toContain('Image page')
        expect(html).toContain('alt="from the map"') // the ui Image found its file in the map
        expect(html).toContain('Terrain')
        expect(html).toContain('Loading 3D')
        expect(html).toContain('An image too large to include')
    })

    it('has no composer, send, reaction or delete', () => {
        expect(html).not.toMatch(/<textarea|<form/i)
        expect(html.toLowerCase()).not.toMatch(/aria-label="(send|delete|react)/)
        expect(html).not.toMatch(/>(Send|Delete|Reply)</)
    })

    it('fetches nothing remote', () => {
        expect(html).not.toMatch(/(?:src|href)="https?:/)
    })
})

describe('hostile content', () => {
    const hostile = [
        '<script>alert(1)</script>',
        '<img src=x onerror=alert(1)>',
        '<iframe srcdoc="<script>1</script>"></iframe>',
        '<svg onload=alert(1)>',
        '[x](javascript:alert(1)) [y](data:text/html,hi) [z](vbscript:x) [ok](https://example.com/a) ![p](https://evil.test/p.png)',
    ].join('\n\n')
    const html = draw(snap({ title: '<img src=x onerror=alert(1)>', turns: [{ id: 'h', role: 'agent', at: 1, parts: [{ kind: 'text', markdown: hostile }] }], media: {} }))

    it('shows markup as text, runs nothing', () => {
        expect(html).not.toMatch(/<(script|iframe|svg)\b/i)
        expect(html).not.toMatch(/<img[^>]*src="x"/)
        expect(html).not.toMatch(/<[^>]*\son(error|load)=/i)
        expect(html).toContain('&lt;img src=x onerror=alert(1)&gt;') // the title, as text
    })

    it('makes javascript:, data: and vbscript: links plain text and keeps https safe', () => {
        expect(html).not.toMatch(/href="(javascript|data|vbscript):/i)
        expect(html).toContain('>x<')
        expect(html).toMatch(/<a href="https:\/\/example.com\/a"[^>]*rel="noopener noreferrer nofollow"/)
    })

    it('does not draw a remote markdown image', () => {
        expect(html).not.toContain('evil.test')
        expect(html).toContain('[image: p]')
    })

    it('refuses a media entry outside the MIME allow-list', () => {
        expect(mediaSrc({ mime: 'image/svg+xml' as never, bytes: 1, data: 'AAAA' })).toBeNull()
        expect(mediaSrc({ mime: 'image/png', bytes: 1, data: '" onerror="x' })).toBeNull()
        expect(mediaFiles({ m: { mime: 'text/html' as never, bytes: 1, data: 'AAAA' } })).toEqual([])
    })

    it('revalidates a ui part: a spec the catalogue refuses is a notice, not a page', () => {
        const bad = draw(snap({}, [{ kind: 'ui', title: 'Bad', spec: { root: 'x', elements: { x: { type: 'Nope', props: {} } } } }]))
        expect(bad).toContain("can&#x27;t be shown")
    })
})

describe('sendMessage in a copy', () => {
    it('is inert and says so; local state actions still work', async () => {
        const store = createStateStore({ n: 1 })
        let blocked = 0
        const h = createViewerHandlers({ store, onBlocked: () => blocked++ })
        await h.sendMessage()
        expect(blocked).toBe(1)
        expect(NOT_IN_A_COPY).toMatch(/copy/)
        await expect(h.run({ fn: 'x' })).rejects.toThrow() // no logic here: refused, never sent anywhere
    })

    it('has landmarks and one level-one heading (T078)', () => {
        const html = renderToString(<SnapshotView snapshot={snap()} />)
        expect(html).toMatch(/<header/)
        expect(html).toMatch(/<main/)
        expect(html).toMatch(/<footer/)
        expect(html.match(/<h1/g)).toHaveLength(1)
        expect(html).toContain('aria-label="Person"')
    })
})
