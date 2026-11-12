import { describe, expect, it } from 'vitest'
import type { SessionFileRecord } from '../../api/frames'
import { MediaBuilder, rewriteSpecFiles, sniffMime, specFileNames, stripGifMetadata, stripJpegMetadata, stripPngMetadata, stripWebpMetadata } from './media'
import { SHARE_LIMITS } from './limits'
import { loadConversation } from './testing'

const fx = loadConversation()
const PNG = Uint8Array.from(Buffer.from(fx.pngBase64, 'base64'))
const file = (filename: string): SessionFileRecord => ({ ...fx.files[0], filename, file_id: `id-${filename}` })

/** SOI, an EXIF APP1 holding "GPS-SECRET", a comment, then a tiny scan. */
const jpeg = (() => {
    const app1 = [0xff, 0xe1, 0x00, 0x0e, ...Buffer.from('Exif\0\0GPS-SECRET').subarray(0, 12)]
    const com = [0xff, 0xfe, 0x00, 0x06, 0x68, 0x69, 0x68, 0x69]
    const sof = [0xff, 0xc0, 0x00, 0x04, 0x00, 0x00]
    return Uint8Array.from([0xff, 0xd8, ...app1, ...com, ...sof, 0xff, 0xda, 0x00, 0x02, 1, 2, 3, 0xff, 0xd9])
})()

describe('sniffMime', () => {
    it('recognises the allow-list by content and nothing else', () => {
        expect(sniffMime(PNG)).toBe('image/png')
        expect(sniffMime(jpeg)).toBe('image/jpeg')
        expect(sniffMime(Uint8Array.from(Buffer.from('GIF89a....')))).toBe('image/gif')
        expect(sniffMime(Uint8Array.from(Buffer.from('RIFF\0\0\0\0WEBPVP8 ')))).toBe('image/webp')
        expect(sniffMime(Uint8Array.from(Buffer.from('<svg xmlns="http://www.w3.org/2000/svg"></svg>')))).toBeNull()
        expect(sniffMime(Uint8Array.from(Buffer.from('<html><script>1</script>')))).toBeNull()
        expect(sniffMime(new Uint8Array())).toBeNull()
    })
})

describe('metadata', () => {
    it('drops EXIF and comments from a JPEG and keeps the scan', () => {
        const out = stripJpegMetadata(jpeg)
        expect(Buffer.from(out).includes('GPS-SECRET')).toBe(false)
        expect(Buffer.from(out).includes('hihi')).toBe(false)
        expect(out.length).toBeLessThan(jpeg.length)
        expect(Array.from(out.subarray(0, 2))).toEqual([0xff, 0xd8])
        expect(Array.from(out.subarray(-5))).toEqual([2, 1, 2, 3, 0xff].slice(0, 0).concat(Array.from(jpeg.subarray(-5))))
    })
    it('returns a damaged JPEG unchanged rather than guess', () => {
        const bad = Uint8Array.from([0xff, 0xd8, 0xff, 0xe1, 0xff, 0xff, 0, 0])
        expect(stripJpegMetadata(bad)).toBe(bad)
    })
    it('drops text chunks from a PNG', () => {
        const chunk = (type: string, data: string) => {
            const body = Buffer.from(type + data)
            const head = Buffer.alloc(4)
            head.writeUInt32BE(data.length)
            return Buffer.concat([head, body, Buffer.alloc(4)])
        }
        const png = Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', '0123456789012'), chunk('tEXt', 'GPS=secret'), chunk('IDAT', 'xx'), chunk('IEND', '')])
        const out = Buffer.from(stripPngMetadata(Uint8Array.from(png)))
        expect(out.includes('GPS=secret')).toBe(false)
        expect(out.includes('IDAT')).toBe(true)
    })
})

describe('MediaBuilder', () => {
    const deps = (bytes: Uint8Array, reencode?: (b: Uint8Array) => Promise<Uint8Array | null>) => ({ fetchFile: async () => ({ bytes }), reencode })
    it('keys an image once however often it is named', async () => {
        const b = new MediaBuilder([file('a.png')], deps(PNG))
        expect(await b.add('a.png')).toEqual({ ok: true, key: 'm1' })
        expect(await b.add('a.png')).toEqual({ ok: true, key: 'm1' })
        expect(Object.keys(b.media)).toEqual(['m1'])
        expect(b.media.m1).toMatchObject({ mime: 'image/png', bytes: PNG.length })
    })
    it('labels a file that is gone, svg named .png, and html', async () => {
        const svg = Uint8Array.from(Buffer.from('<svg><script>1</script></svg>'))
        const b = new MediaBuilder([file('x.png')], deps(svg))
        expect(await b.add('x.png')).toMatchObject({ ok: false, reason: 'unsupported' })
        expect(await b.add('nothere.png')).toMatchObject({ ok: false, reason: 'missing' })
        expect(b.left.map((l) => l.reason)).toEqual(['unsupported', 'missing'])
        expect(b.media).toEqual({})
    })
    it('re-encodes an image over 1 MiB once, and makes a placeholder when that is not enough or not possible', async () => {
        const big = new Uint8Array(SHARE_LIMITS.mediaBytes + 10)
        big.set(Buffer.from('GIF89a'))
        const webp = Uint8Array.from(Buffer.from('RIFF\0\0\0\0WEBPVP8 ' + 'x'.repeat(100)))
        let calls = 0
        const ok = new MediaBuilder([file('b.png')], deps(big, async () => (calls++, webp)))
        expect(await ok.add('b.png')).toMatchObject({ ok: true })
        expect(ok.media.m1.mime).toBe('image/webp')
        expect(calls).toBe(1)
        expect(await new MediaBuilder([file('b.png')], deps(big, async () => big)).add('b.png')).toMatchObject({ ok: false, reason: 'oversized' })
        expect(await new MediaBuilder([file('b.png')], deps(big)).add('b.png')).toMatchObject({ ok: false, reason: 'oversized' })
    })
    it('stops at 100 images', async () => {
        const names = Array.from({ length: 101 }, (_, i) => `f${i}.png`)
        // Each name holds a different picture (same pixels, different trailing byte): content is what is counted.
        const variant = (name: string) => Uint8Array.from([...PNG, Number(name.slice(1, -4))])
        const b = new MediaBuilder(names.map(file), { fetchFile: async (f) => ({ bytes: variant(f.filename) }) })
        for (const n of names) await b.add(n)
        expect(Object.keys(b.media)).toHaveLength(SHARE_LIMITS.mediaItems)
        expect(b.left).toHaveLength(1)
    })
    it('gives a spec key to a lost file that no media entry answers to', async () => {
        const b = new MediaBuilder([], deps(PNG))
        const key = await b.keyFor('gone.png')
        expect(key).toMatch(/^gone-\d+$/)
        expect(b.media[key]).toBeUndefined()
    })
})

describe('spec file names', () => {
    const spec = { root: 'a', elements: { a: { type: 'Gallery', props: { files: ['a.png', 'b.png'] } }, b: { type: 'Canvas2D', props: { shapes: [{ kind: 'image', file: 'a.png' }, { kind: 'rect' }] } }, c: { type: 'Video', props: { file: 'v.mp4', poster: 'a.png' } } } }
    it('finds them all and rewrites them without touching the original', () => {
        expect(specFileNames(spec).sort()).toEqual(['a.png', 'b.png', 'v.mp4'])
        const out = rewriteSpecFiles(spec, new Map([['a.png', 'm1'], ['b.png', 'm2']])) as typeof spec
        expect(out.elements.a.props.files).toEqual(['m1', 'm2'])
        expect((out.elements.b.props.shapes as { file?: string }[])[0].file).toBe('m1')
        expect(out.elements.c.props).toEqual({ file: 'v.mp4', poster: 'm1' })
        expect(spec.elements.a.props.files).toEqual(['a.png', 'b.png'])
    })
    it('tolerates a spec without elements', () => {
        expect(specFileNames({})).toEqual([])
        expect(specFileNames(null)).toEqual([])
    })
})

const u8 = (...parts: (string | number[])[]) => Uint8Array.from(Buffer.concat(parts.map((p) => (typeof p === 'string' ? Buffer.from(p, 'latin1') : Buffer.from(p)))))
const le32 = (n: number) => [n & 255, (n >> 8) & 255, (n >> 16) & 255, (n >>> 24) & 255]
const riffChunk = (type: string, data: number[] | string) => {
    const d = typeof data === 'string' ? Array.from(Buffer.from(data, 'latin1')) : data
    return [...Buffer.from(type, 'latin1'), ...le32(d.length), ...d, ...(d.length & 1 ? [0] : [])]
}
const webp = (...chunks: number[][]) => {
    const body = chunks.flat()
    return Uint8Array.from([...Buffer.from('RIFF'), ...le32(body.length + 4), ...Buffer.from('WEBP'), ...body])
}

describe('WebP and GIF metadata (even under 1 MiB)', () => {
    it('drops EXIF and XMP chunks from a WebP, fixes the RIFF size and the VP8X flags, keeps the picture', () => {
        const vp8x = riffChunk('VP8X', [0x0c, 0, 0, 0, 1, 0, 0, 1, 0, 0])
        const src = webp(vp8x, riffChunk('VP8 ', 'PIXELS-ODD'), riffChunk('EXIF', 'GPS-SECRET-LAT'), riffChunk('XMP ', '<x:xmpmeta>author=bob</x:xmpmeta>'))
        const out = stripWebpMetadata(src)
        const text = Buffer.from(out).toString('latin1')
        expect(text).not.toContain('GPS-SECRET')
        expect(text).not.toContain('author=bob')
        expect(text).toContain('PIXELS-ODD')
        expect(sniffMime(out)).toBe('image/webp')
        expect(new DataView(out.buffer).getUint32(4, true)).toBe(out.length - 8)
        expect(out[12 + 8]).toBe(0) // VP8X flags no longer announce EXIF or XMP
        expect(out.length).toBeLessThan(src.length)
    })
    it('leaves a WebP without metadata, and a damaged one, as they are', () => {
        const plain = webp(riffChunk('VP8 ', 'PIXELS'))
        expect(stripWebpMetadata(plain)).toBe(plain)
        const bad = Uint8Array.from([...Buffer.from('RIFF'), ...le32(500), ...Buffer.from('WEBP'), ...Buffer.from('EXIF'), ...le32(9999)])
        expect(stripWebpMetadata(bad)).toBe(bad)
    })
    it('drops comment and XMP application extensions from a GIF, keeps animation loop, delay and the frame', () => {
        const sub = (s: string) => [s.length, ...Buffer.from(s, 'latin1'), 0]
        const header = u8('GIF89a', [1, 0, 1, 0, 0x80, 0, 0], [0, 0, 0, 255, 255, 255])
        const netscape = [0x21, 0xff, 11, ...Buffer.from('NETSCAPE2.0'), 3, 1, 0, 0, 0]
        const xmp = [0x21, 0xff, 11, ...Buffer.from('XMP DataXMP'), ...sub('<x:xmpmeta>author=bob</x:xmpmeta>')]
        const comment = [0x21, 0xfe, ...sub('taken at 12 Rue Secrète')]
        const gce = [0x21, 0xf9, 4, 0, 10, 0, 0, 0]
        const frame = [0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0, 2, 2, 0x44, 1, 0]
        const src = Uint8Array.from([...header, ...netscape, ...comment, ...xmp, ...gce, ...frame, 0x3b])
        const out = stripGifMetadata(src)
        const text = Buffer.from(out).toString('latin1')
        expect(text).not.toContain('Rue Sec')
        expect(text).not.toContain('author=bob')
        expect(text).toContain('NETSCAPE2.0')
        expect(Array.from(out.subarray(-1))).toEqual([0x3b])
        expect(Array.from(out.subarray(out.length - 1 - frame.length, -1))).toEqual(frame)
        expect(out.length).toBeLessThan(src.length)
    })
    it('returns a damaged GIF unchanged', () => {
        const bad = u8('GIF89a', [1, 0, 1, 0, 0, 0, 0], [0x21, 0xff, 11])
        expect(stripGifMetadata(bad)).toBe(bad)
    })
    it('MediaBuilder strips them from small images too', async () => {
        const src = webp(riffChunk('VP8 ', 'PIXELS'), riffChunk('EXIF', 'GPS-SECRET-LAT'))
        const b = new MediaBuilder([file('p.webp')], { fetchFile: async () => ({ bytes: src }) })
        expect(await b.add('p.webp')).toMatchObject({ ok: true })
        expect(Buffer.from(b.media.m1.data, 'base64').toString('latin1')).not.toContain('GPS-SECRET')
    })
})

describe('MediaBuilder keys by content, not by file name', () => {
    const other = Uint8Array.from([...PNG, 0xaa])
    it('two pasted image.png with different pictures are two entries, each with its own key', async () => {
        const b = new MediaBuilder([], { fetchFile: async () => null })
        const first = await b.add('image.png', PNG)
        const second = await b.add('image.png', other)
        expect(first).toEqual({ ok: true, key: 'm1' })
        expect(second).toEqual({ ok: true, key: 'm2' })
        expect(Object.keys(b.media)).toEqual(['m1', 'm2'])
    })
    it('one picture sent under two names is stored once', async () => {
        const b = new MediaBuilder([], { fetchFile: async () => null })
        expect(await b.add('a.png', PNG)).toEqual({ ok: true, key: 'm1' })
        expect(await b.add('copy of a.png', Uint8Array.from(PNG))).toEqual({ ok: true, key: 'm1' })
        expect(Object.keys(b.media)).toEqual(['m1'])
    })
    it('the same rejected content under two names is left out under each name', async () => {
        const svg = Uint8Array.from(Buffer.from('<svg><script>1</script></svg>'))
        const b = new MediaBuilder([], { fetchFile: async () => null })
        await b.add('x.svg', svg)
        const again = await b.add('y.svg', svg)
        expect(again).toMatchObject({ ok: false, reason: 'unsupported' })
        expect(again.ok === false && again.label).toContain('y.svg')
        expect(b.left.map((l) => l.name)).toEqual(['x.svg', 'y.svg'])
    })
})
