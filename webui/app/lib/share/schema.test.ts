import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { SHARE_LIMITS } from './limits'
import { parseSnapshot, jsonDepth, type Snapshot } from './schema'

const dir = join(__dirname, 'fixtures')
const load = (name: string): unknown => JSON.parse(readFileSync(join(dir, name), 'utf8'))
const ok = (input: unknown): Snapshot => {
    const r = parseSnapshot(input)
    if (!r.ok) throw new Error(`${r.reason}: ${r.message}`)
    return r.snapshot
}
const PNG = (load('conversation.json') as { pngBase64: string }).pngBase64
const base = (over: Record<string, unknown> = {}) => ({
    format_version: 1, created_at: 1, title: 'T', source: { agent_name: 'Ada' }, include_trail: false,
    turns: [{ id: 't1', role: 'agent', at: 1, parts: [{ kind: 'text', markdown: 'hi' }] }], media: {}, rehydration: null, ...over,
})

describe('parseSnapshot: the version gate', () => {
    it('answers needs_newer for a future version', () => {
        const r = parseSnapshot(load('hostile/future-version.json'))
        expect(r).toMatchObject({ ok: false, reason: 'needs_newer' })
    })
    it('answers damaged for missing, zero, fractional and string versions', () => {
        for (const v of [undefined, 0, 1.5, '1', null]) {
            expect(parseSnapshot(base({ format_version: v }))).toMatchObject({ ok: false, reason: 'damaged' })
        }
    })
    it('answers damaged for non-JSON text, arrays, and a cut-off exported file', () => {
        expect(parseSnapshot('not json')).toMatchObject({ ok: false, reason: 'damaged' })
        expect(parseSnapshot('[]')).toMatchObject({ ok: false, reason: 'damaged' })
        expect(parseSnapshot(null)).toMatchObject({ ok: false, reason: 'damaged' })
        const html = readFileSync(join(dir, 'hostile/damaged.vizier.html'), 'utf8')
        const body = /<script type="application\/json" id="vizier-snapshot">([\s\S]*?)<\/script>/.exec(html)![1]
        expect(parseSnapshot(body)).toMatchObject({ ok: false, reason: 'damaged' })
    })
    it('reads a valid document given as text', () => {
        expect(ok(JSON.stringify(base())).turns).toHaveLength(1)
    })
})

describe('parseSnapshot: structure caps', () => {
    it('refuses more turns than the limit and more parts than the limit', () => {
        const turn = { id: 't', role: 'person', at: 1, parts: [] }
        expect(parseSnapshot(base({ turns: Array(SHARE_LIMITS.turns + 1).fill(turn) }))).toMatchObject({ ok: false, reason: 'damaged' })
        expect(ok(base({ turns: Array(SHARE_LIMITS.turns).fill(turn) })).turns).toHaveLength(SHARE_LIMITS.turns)
        const parts = Array(SHARE_LIMITS.parts + 1).fill({ kind: 'text', markdown: 'x' })
        expect(parseSnapshot(base({ turns: [{ id: 't', role: 'person', at: 1, parts }] }))).toMatchObject({ ok: false, reason: 'damaged' })
    })
    it('refuses a malformed turn and a bad role', () => {
        expect(parseSnapshot(base({ turns: [null] })).ok).toBe(false)
        expect(parseSnapshot(base({ turns: [{ id: 't', role: 'system', at: 1, parts: [] }] })).ok).toBe(false)
    })
    it('trims a long title and drops unknown top-level keys', () => {
        const s = ok(base({ title: 'x'.repeat(5000), secret_extra: 1, rehydration: { evil: true } }))
        expect(s.title).toHaveLength(SHARE_LIMITS.titleLength)
        expect(s).not.toHaveProperty('secret_extra')
        expect(s.rehydration).toBeNull()
    })
    it('refuses trail entries in a document that says it has none', () => {
        const turns = [{ id: 't', role: 'agent', at: 1, parts: [], trail: [{ kind: 'tool_call', name: 'x', arguments: {} }] }]
        expect(parseSnapshot(base({ turns })).ok).toBe(false)
        const s = ok(base({ include_trail: true, turns }))
        expect(s.turns[0].trail).toEqual([{ kind: 'tool_call', name: 'x', arguments: {} }])
    })
})

describe('parseSnapshot: hostile fixtures (T005)', () => {
    it('keeps script and handler text as plain text parts, unchanged', () => {
        for (const name of ['script-tag.json', 'onerror.json', 'js-link.json', 'data-block-breakout.json', 'separators.json']) {
            const s = ok(load(`hostile/${name}`))
            expect(s.turns[0].parts.some((p) => p.kind === 'text' || p.kind === 'media' || p.kind === 'placeholder')).toBe(true)
        }
        const t = ok(load('hostile/script-tag.json'))
        expect(t.turns[0].parts[0]).toEqual({ kind: 'text', markdown: expect.stringContaining('<script>alert(1)</script>') })
    })
    it('turns a media key without media into a placeholder', () => {
        const s = ok(load('hostile/onerror.json'))
        expect(s.turns[0].parts[1]).toMatchObject({ kind: 'placeholder', reason: 'missing' })
    })
    it('never admits svg or html as media; the references become placeholders', () => {
        const s = ok(load('hostile/svg-media.json'))
        expect(Object.keys(s.media)).toEqual(['m3'])
        const parts = s.turns[0].parts
        expect(parts[0]).toMatchObject({ kind: 'placeholder', reason: 'unsupported' })
        expect(parts[1]).toMatchObject({ kind: 'placeholder', reason: 'unsupported' })
        expect(parts[2]).toMatchObject({ kind: 'media', key: 'm3' })
    })
    it('turns a part above the text cap into an oversized placeholder and keeps the rest', () => {
        const s = ok(load('hostile/huge-string.json'))
        expect(s.turns[0].parts[0]).toMatchObject({ kind: 'placeholder', reason: 'oversized' })
        expect(s.turns[0].parts[1]).toEqual({ kind: 'text', markdown: 'small' })
    })
    it('turns a spec nested past the limit into a placeholder', () => {
        const s = ok(load('hostile/deep-nesting.json'))
        expect(s.turns[0].parts[0]).toMatchObject({ kind: 'placeholder', reason: 'unsupported' })
        expect(s.turns[0].parts[1]).toEqual({ kind: 'text', markdown: 'after' })
    })
    it('placeholders unknown part kinds and drops unknown keys', () => {
        const s = ok(load('hostile/unknown-kind.json'))
        expect(s.turns[0].parts[0]).toMatchObject({ kind: 'placeholder', reason: 'unsupported' })
        expect(JSON.stringify(s)).not.toContain('<script>')
        expect(s.turns[0].parts[1]).toEqual({ kind: 'text', markdown: 'kept' })
        expect(s).not.toHaveProperty('surprise')
    })
    it('keeps a remote-looking css spec as data (the viewer revalidates), not as a fetch', () => {
        const s = ok(load('hostile/css-remote.json'))
        expect(s.turns[0].parts[0].kind).toBe('ui')
    })
    it('drops __proto__ keys from specs and media keys', () => {
        const raw = JSON.parse('{"kind":"ui","title":"p","spec":{"root":"a","__proto__":{"polluted":1},"elements":{}}}')
        const s = ok(base({ turns: [{ id: 't', role: 'agent', at: 1, parts: [raw] }], media: JSON.parse('{"__proto__":{"mime":"image/png","bytes":1,"data":"AA=="}}') }))
        expect(Object.keys((s.turns[0].parts[0] as { spec: object }).spec)).not.toContain('__proto__')
        expect(Object.keys(s.media)).toEqual([])
        expect(({} as { polluted?: number }).polluted).toBeUndefined()
    })
})

describe('parseSnapshot: media', () => {
    const turns = (keys: string[]) => [{ id: 't', role: 'agent', at: 1, parts: keys.map((key) => ({ kind: 'media', key })) }]
    it('keeps allow-listed raster media and drops unreferenced keys', () => {
        const s = ok(base({ turns: turns(['a']), media: { a: { mime: 'image/png', bytes: 70, data: PNG }, b: { mime: 'image/png', bytes: 70, data: PNG } } }))
        expect(Object.keys(s.media)).toEqual(['a'])
    })
    it('keeps media a ui spec refers to by key', () => {
        const spec = { root: 'a', elements: { a: { type: 'Image', props: { file: 'a' } } } }
        const s = ok(base({ turns: [{ id: 't', role: 'agent', at: 1, parts: [{ kind: 'ui', title: 'x', spec }] }], media: { a: { mime: 'image/png', bytes: 70, data: PNG } } }))
        expect(Object.keys(s.media)).toEqual(['a'])
    })
    it('refuses non-base64 data and oversized data', () => {
        const big = 'A'.repeat(Math.ceil((SHARE_LIMITS.mediaBytes * 4) / 3) + 100)
        const s = ok(base({ turns: turns(['a', 'b']), media: { a: { mime: 'image/png', bytes: 1, data: '<script>' }, b: { mime: 'image/png', bytes: 1, data: big } } }))
        expect(s.turns[0].parts.map((p) => (p as { reason?: string }).reason)).toEqual(['unsupported', 'oversized'])
        expect(s.media).toEqual({})
    })
    it('computes bytes from the data rather than trusting the claim', () => {
        const s = ok(base({ turns: turns(['a']), media: { a: { mime: 'image/png', bytes: 9, data: PNG } } }))
        expect(s.media.a.bytes).toBe(Buffer.from(PNG, 'base64').length)
    })
})

describe('parseSnapshot: unicode (T006)', () => {
    const u = load('unicode.json') as Record<string, string>
    it('round-trips the unicode fixture intact through JSON', () => {
        const parts = [u.rtl, u.combining, u.zwj, u.loneSurrogateEscapes, u.nul, u.longLine].map((markdown) => ({ kind: 'text', markdown }))
        const doc = base({ title: u.title, turns: [{ id: 't', role: 'agent', at: 1, parts }] })
        const s = ok(JSON.parse(JSON.stringify(doc)))
        expect(s.title).toBe(u.title.slice(0, 120))
        expect(s.turns[0].parts.map((p) => (p as { markdown: string }).markdown)).toEqual([u.rtl, u.combining, u.zwj, u.loneSurrogateEscapes, u.nul, u.longLine])
        expect(JSON.parse(JSON.stringify(s))).toEqual(s)
    })
    it('survives a real lone surrogate through stringify', () => {
        const doc = base({ turns: [{ id: 't', role: 'agent', at: 1, parts: [{ kind: 'text', markdown: 'x\ud800y' }] }] })
        const text = JSON.stringify(doc)
        expect(text).toContain('\\ud800')
        expect(ok(text).turns[0].parts[0]).toEqual({ kind: 'text', markdown: 'x\ud800y' })
    })
})

describe('jsonDepth', () => {
    it('counts nesting without recursion', () => {
        let v: unknown = 1
        for (let i = 0; i < 100_000; i++) v = [v]
        expect(jsonDepth(v, 10)).toBeGreaterThan(10)
        expect(jsonDepth({ a: { b: [1] } })).toBe(3)
    })
})

describe('parseSnapshot: the bytes must be the type they claim (D11)', () => {
    const b64 = (s: string) => Buffer.from(s, 'latin1').toString('base64')
    const withMedia = (mime: string, data: string) =>
        base({
            media: { m1: { mime, bytes: 10, data } },
            turns: [{ id: 't1', role: 'agent', at: 1, parts: [{ kind: 'media', key: 'm1', alt: 'x' }] }],
        })

    it('keeps a PNG that is a PNG', () => {
        const s = ok(withMedia('image/png', PNG))
        expect(Object.keys(s.media)).toEqual(['m1'])
        expect(s.turns[0].parts[0]).toMatchObject({ kind: 'media' })
    })
    it.each([
        ['svg bytes claimed as png', 'image/png', b64('<svg xmlns="http://www.w3.org/2000/svg"><script>1</script></svg>')],
        ['html bytes claimed as jpeg', 'image/jpeg', b64('<html><script>alert(1)</script></html>')],
        ['png bytes claimed as webp', 'image/webp', PNG],
        ['png bytes claimed as gif', 'image/gif', PNG],
        ['gif bytes claimed as png', 'image/png', b64('GIF89a\x01\x00\x01\x00\x00\x00\x00;')],
        ['empty data', 'image/png', ''],
        ['a RIFF that is not WebP', 'image/webp', b64('RIFF\x00\x00\x00\x00WAVEfmt ')],
    ])('refuses %s, leaving a placeholder where it was used', (_n, mime, data) => {
        const s = ok(withMedia(mime, data))
        expect(s.media).toEqual({})
        expect(s.turns[0].parts[0]).toMatchObject({ kind: 'placeholder' })
    })
})
