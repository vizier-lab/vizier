import { describe, expect, it } from 'vitest'
import type { ChatMessage } from '../../api/frames'
import { loadConversation, pngDeps } from './testing'
import { parseSnapshot } from './schema'
import { completedLength, readAllHistory, serializeEntries, ShareError, type HistoryReader } from './serialize'

const fx = loadConversation()
const run = (includeTrail: boolean, entries: ChatMessage[] = fx.entries, extra: Record<string, unknown> = {}) =>
    serializeEntries(entries, { agentName: fx.agent_name, title: 'Demo', files: fx.files, media: pngDeps(fx.pngBase64), includeTrail, now: 1, ...extra })

const dump = (o: unknown) => JSON.stringify(o)

describe('serializeEntries: the kinds matrix', () => {
    it('with the trail off: dialogue, images, pages and question cards only', async () => {
        const { snapshot, omittedTrail } = await run(false)
        expect(snapshot.include_trail).toBe(false)
        expect(snapshot.turns.some((t) => t.trail)).toBe(false)
        expect(dump(snapshot)).not.toContain('shell_exec')
        expect(omittedTrail).toBe(3) // ToolCall, ToolResult, ui_components
        const roles = snapshot.turns.map((t) => t.role)
        expect(roles).toEqual(['person', 'agent', 'person', 'person', 'agent', 'person', 'agent'])
        const kinds = snapshot.turns.flatMap((t) => t.parts.map((p) => p.kind))
        expect(kinds).toEqual(expect.arrayContaining(['text', 'media', 'question', 'ui']))
    })

    it('with the trail on: tool calls and results appear, cleaned, and pair up by call id', async () => {
        const { snapshot } = await run(true)
        expect(snapshot.include_trail).toBe(true)
        const trail = snapshot.turns.flatMap((t) => t.trail ?? [])
        const shell = trail.find((t) => t.name === 'shell_exec')!
        expect(shell.result).toContain('[removed:')
        expect(dump(shell.arguments)).not.toContain('C:\\Users\\alice')
        expect(trail.map((t) => t.name)).toEqual(['shell_exec', 'ui_components'])
        expect(trail.find((t) => t.name === 'ui_components')).toMatchObject({ ok: true })
    })

    it.each([false, true])('never carries recall, reasoning, sensitive asks, checkpoints or commands (trail=%s)', async (trail) => {
        const { snapshot, removals } = await run(trail)
        const text = dump(snapshot)
        const s = fx.seeds
        for (const marker of [...s.recall, ...s.reasoning, ...s.sensitive, ...s.checkpoint, ...s.command]) expect(text).not.toContain(marker)
        expect(removals.recall).toBe(1)
        expect(removals.reasoning).toBe(1)
        expect(removals.credentials).toBeGreaterThanOrEqual(1 + 1) // the sensitive ask and at least one key
    })

    it('removes answerer from question sets but keeps the answers', async () => {
        const { snapshot } = await run(false)
        const q = snapshot.turns.flatMap((t) => t.parts).find((p) => p.kind === 'question')!
        expect(dump(snapshot)).not.toContain('ANSWERER-MARKER')
        expect(q).toMatchObject({ kind: 'question', title: 'Which one?', state: 'answered' })
        const items = (q as { items: { id: string; answer?: { value?: unknown; label?: string } }[] }).items
        expect(items[0].answer).toMatchObject({ value: 'red', label: 'Red' })
        expect(items[1].answer).toMatchObject({ value: true })
    })

    it('treats an unknown kind as never', async () => {
        const odd = { uid: 'z', vizier_session: fx.entries[0].vizier_session, content: { FutureThing: { text: 'LEAK-MARKER' } } } as unknown as ChatMessage
        for (const trail of [false, true]) expect(dump((await run(trail, [...fx.entries, odd])).snapshot)).not.toContain('LEAK-MARKER')
    })

    it('ignores request kinds that are not what a person said', async () => {
        const silent = { ...fx.entries[0], uid: 's', content: { Request: { timestamp: 'x', user: 'u', content: { silent_read: 'SILENT-MARKER' } } } } as ChatMessage
        expect(dump((await run(false, [silent, ...fx.entries])).snapshot)).not.toContain('SILENT-MARKER')
    })
})

describe('serializeEntries: content', () => {
    it('keeps ordinary identifiers and words intact', async () => {
        const text = dump((await run(false)).snapshot)
        expect(text).toContain('3f786850e387550fdab836ed7e6dc881de23001b')
        expect(text).toContain('123e4567-e89b-12d3-a456-426614174000')
        expect(text).toContain('a token is a unit of text')
    })

    it('turns ui_render into a ui part with the saved state and media keys, not file names', async () => {
        const { snapshot } = await run(false)
        const ui = snapshot.turns.flatMap((t) => t.parts).find((p) => p.kind === 'ui')!
        expect(ui).toMatchObject({ kind: 'ui', title: 'Dashboard', state: { count: 7 } })
        const spec = (ui as unknown as { spec: { elements: Record<string, { props: Record<string, unknown> }> } }).spec
        // photo.png was attached first (m1); chart.png comes with the page (m2).
        expect(spec.elements.pic.props.file).toBe('m2')
        expect((spec.elements.canvas.props.shapes as { file?: string }[])[1].file).toBe('m1')
        expect(Object.keys(snapshot.media).sort()).toEqual(['m1', 'm2'])
        expect(dump(spec)).toContain('Scene3D')
        expect(dump(spec)).toContain('Canvas2D')
        expect(dump(spec)).not.toContain('/home/alice')
    })

    it('turns attachments into media parts and a missing file into a placeholder', async () => {
        const { snapshot, placeholders, removals } = await run(false, fx.entries, { files: [] })
        expect(Object.keys(snapshot.media)).toEqual([])
        expect(placeholders.map((p) => p.reason)).toEqual(['missing', 'missing'])
        expect(removals.media).toBe(2)
        const first = snapshot.turns[0].parts
        expect(first.map((p) => p.kind)).toEqual(['text', 'placeholder'])
    })

    it('puts no person name, id or agent id in the document', async () => {
        const text = dump((await run(false)).snapshot)
        for (const s of ['alice"', 'agent-1', 'topic-1', 'u1"']) expect(text).not.toContain(s)
    })

    it('produces a document the viewer accepts, unchanged', async () => {
        for (const trail of [false, true]) {
            const { snapshot } = await run(trail)
            const parsed = parseSnapshot(JSON.parse(JSON.stringify(snapshot)))
            expect(parsed.ok && parsed.snapshot).toEqual(snapshot)
        }
    })

    it('falls back to a plain title and cleans a title with markup or a path', async () => {
        expect((await run(false, fx.entries, { title: '   ' })).snapshot.title).toBe('Shared conversation')
        const t = (await run(false, fx.entries, { title: `<img src=x onerror=1> /home/alice/${'x'.repeat(200)}` })).snapshot.title
        expect(t.length).toBeLessThanOrEqual(120)
        expect(t).not.toContain('/home/alice')
    })

    it('splits a turn at 64 parts and refuses more than 5,000 turns', async () => {
        const many = Array.from({ length: 70 }, (_, i) => ({ ...fx.entries[5], uid: `a${i}` }))
        const { snapshot } = await run(false, many, { turnInProgress: false })
        expect(snapshot.turns.map((t) => t.parts.length)).toEqual([64, 6])
        const request = (i: number) => ({ ...fx.entries[14], uid: `r${i}` })
        const huge = Array.from({ length: 5001 }, (_, i) => request(i))
        await expect(run(false, huge, { turnInProgress: false })).rejects.toMatchObject({ code: 'too_many_turns' })
    })

    it('cuts a message above the per-part limit and says so', async () => {
        const big = { ...fx.entries[5], content: { AssistantMessage: 'é'.repeat(200_000) } }
        const r = await run(false, [big], { turnInProgress: false })
        expect(r.truncated).toBe(1)
        expect(r.snapshot.turns[0].parts[0]).toMatchObject({ kind: 'text' })
    })
})

describe('turn in progress', () => {
    const e = fx.entries
    it('is detected from the shape and left out', async () => {
        const partial = [...e, { ...e[0], uid: 'new-req', content: { Request: { timestamp: 'x', user: 'alice', content: { chat: 'STILL-RUNNING-MARKER' } } } } as ChatMessage, { ...e[3], uid: 'new-call' }]
        expect(completedLength(partial)).toBe(e.length)
        const r = await run(false, partial)
        expect(r.turnInProgress).toBe(true)
        expect(dump(r.snapshot)).not.toContain('STILL-RUNNING-MARKER')
    })
    it('is not reported for a finished conversation, and can be forced', async () => {
        expect((await run(false)).turnInProgress).toBe(false)
        expect((await run(false, e, { turnInProgress: true })).turnInProgress).toBe(true)
    })
})

describe('readAllHistory', () => {
    const make = (n: number): ChatMessage[] => Array.from({ length: n }, (_, i) => ({ uid: `e${i}`, seq: i, vizier_session: fx.entries[0].vizier_session, content: { AssistantMessage: `m${i}` } }))
    const reader = (all: ChatMessage[]): HistoryReader & { calls: number } => ({
        calls: 0,
        async page({ before_seq, limit }) {
            this.calls++
            const end = before_seq == null ? all.length : all.findIndex((x) => x.seq === before_seq)
            return all.slice(Math.max(0, end - limit), end)
        },
    })
    it('reads every page until exhausted, oldest first, without duplicates', async () => {
        const all = make(1234)
        const r = reader(all)
        const got = await readAllHistory(r, { pageSize: 200 })
        expect(got.map((x) => x.uid)).toEqual(all.map((x) => x.uid))
        expect(r.calls).toBe(7)
    })
    it('stops when a page repeats (a server that ignores the cursor)', async () => {
        const all = make(10)
        const got = await readAllHistory({ page: async () => all }, { pageSize: 5 })
        expect(got).toHaveLength(10)
    })
    it('stops on an empty history', async () => {
        expect(await readAllHistory({ page: async () => [] })).toEqual([])
    })
    it('refuses a history longer than the turn cap', async () => {
        const reqs = Array.from({ length: 30 }, (_, i) => ({ ...fx.entries[0], uid: `q${i}`, seq: i }))
        await expect(readAllHistory(reader(reqs), { pageSize: 10, maxRequests: 25 })).rejects.toBeInstanceOf(ShareError)
    })
})
