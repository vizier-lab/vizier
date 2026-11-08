import { beforeEach, describe, expect, it, vi } from 'vitest'

const { post } = vi.hoisted(() => ({ post: vi.fn(async (..._a: unknown[]) => ({ share_id: 'x'.repeat(21), url_path: '/share/x', created_at: 1 })) }))
vi.mock('../../api/client', async (orig) => ({ ...(await orig<typeof import('../../api/client')>()), api: { get: vi.fn(), post, put: vi.fn(), patch: vi.fn(), delete: vi.fn() } }))

import { buildCreateRequest, createShare } from '../../api/shares'
import { buildPreview, formatBytes, snapshotBytes } from './preview'
import { serializeEntries, ShareError, type SerializeResult } from './serialize'
import { loadConversation, pngDeps } from './testing'
import { SHARE_LIMITS } from './limits'

const fx = loadConversation()
const make = (includeTrail = false, entries = fx.entries, extra: Record<string, unknown> = {}) =>
    serializeEntries(entries, { agentName: 'Ada', title: 'Demo', files: fx.files, media: pngDeps(fx.pngBase64), includeTrail, now: 1, ...extra })

beforeEach(() => post.mockClear())

describe('buildPreview: counts', () => {
    it('counts messages, images, pages, questions and the exact size', async () => {
        const r = await make()
        const p = buildPreview(r)
        expect(p.images).toBe(Object.keys(r.snapshot.media).length)
        expect(p.pages).toBe(1)
        expect(p.questions).toBe(1)
        expect(p.messages).toBeGreaterThanOrEqual(5)
        expect(p.sizeBytes).toBe(Buffer.byteLength(JSON.stringify(r.snapshot)))
        expect(p.canShare).toBe(true)
        expect(p.refusal).toBeNull()
    })
    it('says what was left out, and that the trail is not included (default off)', async () => {
        const p = buildPreview(await make())
        expect(p.includeTrail).toBe(false)
        expect(p.notes.join(' ')).toMatch(/activity.*not included/)
        expect(p.omittedTrail).toBe(3)
    })
    it('says the trail is included when it is on', async () => {
        const p = buildPreview(await make(true))
        expect(p.includeTrail).toBe(true)
        expect(p.trailEntries).toBe(2)
        expect(p.notes.join(' ')).toMatch(/activity is included \(2 steps\)/)
    })
    it('notes a turn in progress', async () => {
        const p = buildPreview(await make(false, fx.entries, { turnInProgress: true }))
        expect(p.turnInProgress).toBe(true)
        expect(p.notes.join(' ')).toContain('still being written')
    })
    it('lists files that became placeholders', async () => {
        const p = buildPreview(await make(false, fx.entries, { files: [] }))
        expect(p.placeholders.map((x) => x.name)).toEqual(['photo.png', 'chart.png'])
        expect(p.removals.media).toBe(2)
    })
})

describe('buildPreview: removals equal removals made (TP-22..24)', () => {
    it('each kind in the preview equals what the serializer counted, and what the request will carry', async () => {
        const r = await make()
        const p = buildPreview(r)
        expect(p.removals).toEqual(r.removals)
        expect(p.removals.recall).toBe(1)
        expect(p.removals.reasoning).toBe(1)
        expect(p.removals.paths).toBeGreaterThan(0)
        expect(p.removals.credentials).toBeGreaterThan(1)
        expect(p.removed.map((x) => x.kind).sort()).toEqual(['credentials', 'paths', 'reasoning', 'recall'])
        expect(p.removed.reduce((n, x) => n + x.count, 0)).toBe(Object.values(r.removals).reduce((a, b) => a + b, 0))
        const body = buildCreateRequest({ agentId: 'a', topicId: 't', title: 'Demo', audience: 'public' }, r)
        expect(body.removals).toEqual(p.removals)
        expect(body.removals).not.toBe(r.removals)
    })
    it('counts the markers actually present in the document', async () => {
        const r = await make(true)
        const markers = JSON.stringify(r.snapshot).match(/\[removed: [a-z ]+\]/g) ?? []
        // every marker is a credential removal; sensitive asks are counted but leave no marker
        expect(r.removals.credentials).toBeGreaterThanOrEqual(markers.length)
        expect(r.removals.credentials - markers.length).toBe(1)
    })
})

describe('buildPreview: refusals', () => {
    it('refuses an empty conversation with a reason and no options to offer', async () => {
        const p = buildPreview(await make(false, []))
        expect(p.refusal).toMatchObject({ reason: 'empty', options: [] })
        expect(p.canShare).toBe(false)
    })
    it('refuses a conversation with only hidden kinds', async () => {
        const hidden = fx.entries.filter((e) => e.content.Recall || e.content.Command)
        expect(buildPreview(await make(false, hidden)).refusal?.reason).toBe('empty')
    })
    it('refuses above the size limit with the size, the limit and the ways to shrink it; nothing is cut', async () => {
        const r = await make(true)
        const filler = 'x'.repeat(20_000)
        const turns = Array.from({ length: 300 }, () => ({ kind: 'text' as const, markdown: filler }))
        r.snapshot.turns.push({ id: 'big', role: 'agent', at: 1, parts: turns.slice(0, 64), trail: [{ kind: 'tool_call', name: 'x', arguments: {}, result: filler }] })
        for (let i = 0; i < 4; i++) r.snapshot.turns.push({ id: `b${i}`, role: 'agent', at: 1, parts: turns.slice(0, 64) })
        const before = JSON.stringify(r.snapshot)
        const p = buildPreview(r)
        expect(p.sizeBytes).toBeGreaterThan(p.limitBytes)
        expect(p.refusal?.reason).toBe('too_large')
        expect(p.refusal?.message).toContain(formatBytes(p.sizeBytes))
        expect(p.refusal?.message).toContain(formatBytes(p.limitBytes))
        expect(p.refusal?.options.join(' ')).toMatch(/activity/)
        expect(p.refusal?.options.join(' ')).toMatch(/images/)
        expect(JSON.stringify(r.snapshot)).toBe(before)
        expect(() => buildCreateRequest({ agentId: 'a', topicId: 't', title: 'x', audience: 'public' }, r)).toThrow(ShareError)
    })
    it('keeps the limit below the body limit by the request wrapper', () => {
        expect(buildPreview({ snapshot: { turns: [], media: {}, include_trail: false } as never, removals: { credentials: 0, paths: 0, recall: 0, reasoning: 0, media: 0 }, placeholders: [], omittedTrail: 0, turnInProgress: false, truncated: 0 }).limitBytes).toBe(SHARE_LIMITS.bodyBytes - SHARE_LIMITS.envelopeBytes)
        expect(snapshotBytes({ a: 'é' } as never)).toBe(10)
    })
})

describe('creating, and cancelling (TP-25)', () => {
    const choice = { agentId: 'a1', topicId: 't1', title: 'Demo', audience: 'public' as const }
    let result: SerializeResult
    beforeEach(async () => {
        result = await make()
    })

    it('makes no network call while previewing or when the person cancels', async () => {
        const preview = buildPreview(result)
        expect(preview.canShare).toBe(true)
        buildCreateRequest(choice, result) // building the request is pure
        // Cancel: the dialog simply stops here.
        expect(post).not.toHaveBeenCalled()
    })

    it('posts exactly the previewed document, removals and trail flag once on confirm', async () => {
        const preview = buildPreview(result)
        await createShare(buildCreateRequest(choice, result))
        expect(post).toHaveBeenCalledTimes(1)
        const [path, body] = post.mock.calls[0] as unknown as [string, ReturnType<typeof buildCreateRequest>]
        expect(path).toBe('/shares')
        expect(body.removals).toEqual(preview.removals)
        expect(body.include_trail).toBe(preview.includeTrail)
        expect(body.include_trail).toBe(body.snapshot.include_trail)
        expect(Buffer.byteLength(JSON.stringify(body.snapshot))).toBe(preview.sizeBytes)
        expect(body).not.toHaveProperty('people')
    })

    it('validates the people rule on the client too', () => {
        expect(() => buildCreateRequest({ ...choice, audience: 'specific' }, result)).toThrow(/at least one/)
        expect(() => buildCreateRequest({ ...choice, audience: 'specific', people: Array.from({ length: 51 }, (_, i) => `u${i}`) }, result)).toThrow(/at most 50/)
        expect(() => buildCreateRequest({ ...choice, audience: 'signed_in', people: ['u1'] }, result)).toThrow(ShareError)
        expect(buildCreateRequest({ ...choice, audience: 'specific', people: ['u1', 'u1', 'u2'] }, result).people).toEqual(['u1', 'u2'])
    })

    it('trims the title to 120 and falls back when empty', () => {
        expect(buildCreateRequest({ ...choice, title: 'x'.repeat(500) }, result).title).toHaveLength(120)
        expect(buildCreateRequest({ ...choice, title: '  ' }, result).title).toBe('Demo')
    })
})
