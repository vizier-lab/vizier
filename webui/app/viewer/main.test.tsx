// T070 / TP-75: the real entry (`main.tsx`) is imported and run against the hostile fixtures. The DOM
// is a stub with just what the entry touches; React's `createRoot` is replaced by a recorder, and what
// it was asked to draw is rendered to a string and inspected.
import { readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { renderToString } from 'react-dom/server'
import type { ReactElement } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { embedJson } from '../lib/share/embed'
import { DAMAGED, OFFLINE_BANNER } from './Neutral'
import { READ_ONLY_LINE } from './SnapshotView'

let drawn: ReactElement | null = null
let renders = 0
vi.mock('react-dom/client', () => ({
    createRoot: () => ({
        render: (el: ReactElement) => {
            drawn = el
            renders++
        },
    }),
}))

const fixtures = join(__dirname, '../lib/share/fixtures')
const hostileDir = join(fixtures, 'hostile')

const validDoc = {
    format_version: 1,
    created_at: Date.UTC(2026, 10, 8),
    title: 'Trip planning',
    source: { agent_name: 'Atlas' },
    include_trail: false,
    turns: [{ id: 't1', role: 'person', at: 1, parts: [{ kind: 'text', markdown: 'Plan a **3-day** trip' }] }],
    media: {},
    rehydration: null,
}

interface FakeTag {
    tagName: string
    textContent: string | null
}
const script = (text: string | null): FakeTag => ({ tagName: 'SCRIPT', textContent: text })

const doc = { title: 'Vizier', documentElement: { style: {} }, getElementById: vi.fn(), querySelectorAll: vi.fn() }

/** Runs the entry once against a page holding these snapshot tags; returns the HTML it drew. */
async function boot(tags: FakeTag[], withRoot = true): Promise<string> {
    drawn = null
    renders = 0
    doc.title = 'Vizier'
    doc.getElementById.mockImplementation((id: string) => (id === 'root' && withRoot ? {} : null))
    doc.querySelectorAll.mockImplementation((sel: string) => (sel.includes('vizier-snapshot') ? tags : []))
    vi.resetModules()
    await import('./main')
    return drawn ? renderToString(drawn) : ''
}

beforeEach(() => {
    vi.stubGlobal('document', doc)
    delete (globalThis as { __pwned?: boolean }).__pwned
})
afterEach(() => vi.unstubAllGlobals())

const jsonFixtures = readdirSync(hostileDir).filter((f) => f.endsWith('.json'))

describe('main.tsx on the hostile fixtures', () => {
    it('finds the fixtures', () => {
        expect(jsonFixtures.length).toBeGreaterThanOrEqual(10)
    })

    for (const name of jsonFixtures) {
        it(`${name}: draws once, as inert text or a neutral page, and executes nothing`, async () => {
            const embedded = embedJson(JSON.parse(readFileSync(join(hostileDir, name), 'utf8')))
            const html = await boot([script(embedded)])
            expect(renders).toBe(1)
            expect(html.length).toBeGreaterThan(0)
            // Nothing from the conversation became markup that can run.
            expect(html).not.toMatch(/<script/i)
            expect(html).not.toMatch(/<(?:img|svg|iframe|object|embed|link|style|base|form)[^>]*\son\w+\s*=/i)
            expect(html).not.toMatch(/\s(?:href|src)="\s*(?:javascript|vbscript):/i)
            expect(html).not.toMatch(/<(?:iframe|object|embed|base|link|form)\b/i)
            expect((globalThis as { __pwned?: boolean }).__pwned).toBeUndefined()
        })
    }

    it('a future format version says to open it with a newer Vizier', async () => {
        const html = await boot([script(embedJson(JSON.parse(readFileSync(join(hostileDir, 'future-version.json'), 'utf8'))))])
        expect(html).toContain('newer version')
        expect(html).not.toContain(DAMAGED)
    })

    it('a title with markup is set as text on the tab and escaped on the page', async () => {
        const html = await boot([script(embedJson(JSON.parse(readFileSync(join(hostileDir, 'script-tag.json'), 'utf8'))))])
        expect(html).not.toMatch(/<script/i)
        expect(doc.title.endsWith(' · Vizier')).toBe(true)
        expect(doc.title.length).toBeLessThanOrEqual(120 + ' · Vizier'.length)
    })

    it('the cut-off exported file is reported as damaged', async () => {
        const page = readFileSync(join(hostileDir, 'damaged.vizier.html'), 'utf8')
        const body = /<script type="application\/json" id="vizier-snapshot">([\s\S]*?)<\/script>/.exec(page)![1]
        expect(await boot([script(body)])).toContain(DAMAGED)
    })

    it('a valid conversation is drawn read-only', async () => {
        const html = await boot([script(embedJson(validDoc))])
        expect(html).toContain('Trip planning')
        expect(html).toContain(OFFLINE_BANNER) // FR-035: the file says it is an unverified copy
        expect(html).toContain(READ_ONLY_LINE.replace(/'/g, '&#x27;'))
        expect(html).not.toMatch(/<textarea|contenteditable/i)
    })
})

describe('main.tsx: the snapshot tag', () => {
    const valid = () => script(embedJson(validDoc))

    it('rejects two snapshot tags, even when both are valid (TP-75)', async () => {
        const html = await boot([valid(), valid()])
        expect(renders).toBe(1)
        expect(html).toContain(DAMAGED)
        expect(html).not.toContain(READ_ONLY_LINE.replace(/'/g, '&#x27;'))
    })

    it('rejects a valid tag followed by a hostile one', async () => {
        const evil = script(embedJson(JSON.parse(readFileSync(join(hostileDir, 'onerror.json'), 'utf8'))))
        expect(await boot([valid()])).not.toContain(DAMAGED) // the control: one valid tag is fine
        expect(await boot([valid(), evil])).toContain(DAMAGED)
        expect(await boot([evil, valid()])).toContain(DAMAGED)
    })

    it('rejects no tag, an empty tag, a tag that is not a script, and text that is not JSON', async () => {
        expect(await boot([])).toContain(DAMAGED)
        expect(await boot([script('')])).toContain(DAMAGED)
        expect(await boot([script(null)])).toContain(DAMAGED)
        expect(await boot([{ tagName: 'DIV', textContent: valid().textContent }])).toContain(DAMAGED)
        expect(await boot([script('{ broken')])).toContain(DAMAGED)
    })

    it('does nothing at all on a page without a root element', async () => {
        expect(await boot([valid()], false)).toBe('')
        expect(renders).toBe(0)
    })
})

describe('main.tsx keeps the single-file bundle lean', () => {
    const imports = (file: string) => [...readFileSync(join(__dirname, file), 'utf8').matchAll(/^import[^'"]*['"]([^'"]+)['"]/gm)].map((m) => m[1])

    it.each(['main.tsx', 'Neutral.tsx', 'SnapshotView.tsx', 'SharedUi.tsx', 'media.ts', '../lib/share/schema.ts', '../lib/share/sniff.ts'])('%s imports no route, API client or transport', (file) => {
        for (const spec of imports(file)) {
            expect(spec, `${file} -> ${spec}`).not.toMatch(/(^|\/)routes\//)
            expect(spec, `${file} -> ${spec}`).not.toMatch(/(^|\/)api\/(?!frames$)/)
            expect(spec, `${file} -> ${spec}`).not.toMatch(/transport|stores\/session/)
        }
    })
})
