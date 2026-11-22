// A page the agent drew in HTML (spec 063) in a shared copy: it is carried as a `page` part, only
// its last version, and it is text — never markup — so the viewer can frame it under its own policy.
import { describe, expect, it } from 'vitest'
import type { ChatMessage } from '../../api/frames'
import { SHARE_LIMITS } from './limits'
import { buildPreview } from './preview'
import { parseSnapshot, type PagePart, type Snapshot } from './schema'
import { completedLength, serializeEntries, type PageHtmlFetcher } from './serialize'
import { loadConversation } from './testing'

const fx = loadConversation()
const SESSION = fx.entries[0].vizier_session
let n = 0
const entry = (content: ChatMessage['content']): ChatMessage => ({
    uid: `p${++n}`,
    seq: n,
    timestamp: new Date(1_700_000_000_000 + n * 1000).toISOString(),
    vizier_session: SESSION,
    content,
})

const PAGE_HTML = '<!doctype html><html><head><title>Chess</title></head><body><div id="board">♞</div><script>document.title="ok"</script></body></html>'
const page = (page_id: string, version: number, title = 'Chess') => entry({ UiPage: { page_id, version, title } })

const run = (entries: ChatMessage[], pageHtml?: PageHtmlFetcher) =>
    serializeEntries(entries, {
        agentName: 'Ada',
        title: 'Demo',
        includeTrail: false,
        now: 1,
        turnInProgress: false,
        ...(pageHtml ? { pageHtml } : {}),
    })

const htmlFor = (map: Record<string, string>): PageHtmlFetcher => async ({ pageId, version }) => map[`${pageId}:${version}`] ?? null

const conversation = (...pages: ChatMessage[]) => [
    entry({ Request: { timestamp: 'x', user: 'u', content: { chat: 'can you make an interactable chess?' } } }),
    entry({ Response: { timestamp: 'x', content: { message: { content: 'Here is a board.' } } } }),
    ...pages,
]

const pageParts = (snapshot: Snapshot): PagePart[] => snapshot.turns.flatMap((t) => t.parts).filter((p): p is PagePart => p.kind === 'page')

describe('serializeEntries: an HTML page (spec 063)', () => {
    it('carries the page as a part, so the agent turn is not empty', async () => {
        const { snapshot } = await run(conversation(page('pg_1', 1)), htmlFor({ 'pg_1:1': PAGE_HTML }))
        const agent = snapshot.turns[1]
        expect(agent.role).toBe('agent')
        expect(agent.parts.map((p) => p.kind)).toEqual(['text', 'page'])
        expect(pageParts(snapshot)).toEqual([{ kind: 'page', title: 'Chess', html: PAGE_HTML, version: 1 }])
    })

    it('keeps only the last version of a page: the drafts before it are not copied', async () => {
        const versions = { 'pg_1:1': '<p>draft</p>', 'pg_1:2': '<p>fixed</p>', 'pg_2:4': '<p>other</p>' }
        const { snapshot } = await run(
            conversation(page('pg_1', 1), page('pg_1', 2), page('pg_2', 3), page('pg_2', 4)),
            htmlFor(versions)
        )
        const parts = pageParts(snapshot)
        expect(parts.map((p) => [p.title, p.version, p.html])).toEqual([
            ['Chess', 2, '<p>fixed</p>'],
            ['Chess', 4, '<p>other</p>'],
        ])
        expect(JSON.stringify(snapshot)).not.toContain('draft')
    })

    it('names a page it could not read instead of leaving a hole', async () => {
        const { snapshot } = await run(conversation(page('pg_1', 1, 'Board')), async () => null)
        expect(pageParts(snapshot)).toEqual([])
        expect(snapshot.turns.flatMap((t) => t.parts)).toContainEqual({
            kind: 'placeholder',
            reason: 'missing',
            label: 'Interactive page “Board” could not be included',
        })
    })

    it('refuses an empty document and one over the cap, and keeps the turn', async () => {
        const big = `<p>${'x'.repeat(SHARE_LIMITS.pageBytes)}</p>`
        const { snapshot } = await run(conversation(page('pg_1', 1, 'Small'), page('pg_2', 1, 'Big')), htmlFor({ 'pg_1:1': '   ', 'pg_2:1': big }))
        const parts = snapshot.turns.flatMap((t) => t.parts)
        expect(parts.filter((p) => p.kind === 'placeholder')).toEqual([
            expect.objectContaining({ reason: 'missing' }),
            expect.objectContaining({ reason: 'oversized', label: 'Interactive page “Big” (too large to include)' }),
        ])
        expect(parts.some((p) => p.kind === 'page')).toBe(false)
        expect(parts.some((p) => p.kind === 'text')).toBe(true)
    })

    it('cleans a key written into the page, and counts it', async () => {
        const key = 'sk-proj-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
        const { snapshot, removals } = await run(conversation(page('pg_1', 1)), htmlFor({ 'pg_1:1': `<script>const k="${key}"</script>` }))
        const html = pageParts(snapshot)[0].html
        expect(html).not.toContain(key)
        expect(html).toContain('[removed:')
        expect(removals.credentials).toBeGreaterThanOrEqual(1)
    })

    it('counts a page in the preview, beside the json-render pages', async () => {
        const result = await run(conversation(page('pg_1', 1)), htmlFor({ 'pg_1:1': PAGE_HTML }))
        expect(buildPreview(result).pages).toBe(1)
    })

    it('treats a page as the end of its turn, so the last turn is complete', async () => {
        const entries = conversation(page('pg_1', 1))
        expect(completedLength(entries)).toBe(entries.length)
    })
})

describe('parseSnapshot: a page part is hostile input', () => {
    const doc = (part: unknown) => ({
        format_version: 1,
        created_at: 1,
        title: 'T',
        source: { agent_name: 'Ada' },
        include_trail: false,
        turns: [{ id: 't1', role: 'agent', at: 1, parts: [part] }],
        media: {},
        rehydration: null,
    })
    const ok = (input: unknown): Snapshot => {
        const r = parseSnapshot(input)
        if (!r.ok) throw new Error(r.message)
        return r.snapshot
    }

    it('keeps a page as text, script and all, and drops unknown keys', async () => {
        const s = ok(doc({ kind: 'page', title: 'Chess', html: PAGE_HTML, version: 2, onclick: 'alert(1)' }))
        const part = s.turns[0].parts[0] as PagePart
        expect(part).toEqual({ kind: 'page', title: 'Chess', html: PAGE_HTML, version: 2 })
        expect(Object.keys(part)).not.toContain('onclick')
    })

    it('turns a page above the cap into an oversized placeholder', () => {
        const s = ok(doc({ kind: 'page', title: 'Chess', html: 'x'.repeat(SHARE_LIMITS.pageBytes + 1) }))
        expect(s.turns[0].parts[0]).toMatchObject({ kind: 'placeholder', reason: 'oversized' })
    })

    it('refuses a page whose html is not a string', () => {
        for (const html of [1, null, { text: '<p>x</p>' }, ['<p>x</p>']]) {
            const s = ok(doc({ kind: 'page', title: 'Chess', html }))
            expect(s.turns[0].parts[0]).toMatchObject({ kind: 'placeholder', reason: 'unsupported' })
        }
    })

    it('drops a version that is not a whole number', () => {
        const s = ok(doc({ kind: 'page', title: 'Chess', html: '<p>x</p>', version: 'two' }))
        expect(s.turns[0].parts[0]).toEqual({ kind: 'page', title: 'Chess', html: '<p>x</p>' })
    })
})
