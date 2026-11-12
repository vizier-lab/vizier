// Images of a conversation, made safe to embed (research R3): raster allow-list by content (not by the
// name or the claimed type), metadata dropped, big images re-encoded once, anything else a labelled
// placeholder. The output is the snapshot's `media` map and the name -> key rewrite for specs.
import type { SessionFileRecord } from '../../api/frames'
import { transport } from '../../api/transport'
import { fileReferences, findFile } from '../generativeUi/media'
import { isAllowedMime, MEDIA_MIME_ALLOW, SHARE_LIMITS, type MediaMime } from './limits'
import type { MediaEntry, PlaceholderReason } from './schema'
import { sniffMime } from './sniff'

export interface FetchedFile { bytes: Uint8Array; mime?: string }
export interface MediaDeps {
    /** The bytes of a stored file, or null when it cannot be read. */
    fetchFile(file: SessionFileRecord): Promise<FetchedFile | null>
    /** One re-encode to WebP at lower quality; null when the platform cannot. */
    reencode?(bytes: Uint8Array, mime: MediaMime): Promise<Uint8Array | null>
}

export type MediaOutcome = { ok: true; key: string } | { ok: false; reason: PlaceholderReason; label: string }

export { sniffMime }

/** A JPEG without its APP1 (EXIF, XMP), APP13 (IPTC) and comment segments: the GPS position and camera go, the pixels do not change. */
export function stripJpegMetadata(b: Uint8Array): Uint8Array {
    const out: number[][] = []
    let i = 2
    const head = [b[0], b[1]]
    while (i + 4 <= b.length) {
        if (b[i] !== 0xff) break
        const marker = b[i + 1]
        if (marker === 0xd9) break
        if (marker === 0xda) break // start of scan: the rest is image data
        const len = (b[i + 2] << 8) | b[i + 3]
        if (len < 2 || i + 2 + len > b.length) return b
        if (marker !== 0xe1 && marker !== 0xed && marker !== 0xfe) out.push([i, i + 2 + len])
        i += 2 + len
    }
    const parts: Uint8Array[] = [Uint8Array.from(head)]
    for (const [s, e] of out) parts.push(b.subarray(s, e))
    parts.push(b.subarray(i))
    return concat(parts)
}

const PNG_DROP = new Set(['eXIf', 'tEXt', 'iTXt', 'zTXt', 'tIME'])

/** A PNG without its text and EXIF chunks. */
export function stripPngMetadata(b: Uint8Array): Uint8Array {
    const parts: Uint8Array[] = [b.subarray(0, 8)]
    let i = 8
    while (i + 12 <= b.length) {
        const len = ((b[i] << 24) | (b[i + 1] << 16) | (b[i + 2] << 8) | b[i + 3]) >>> 0
        const type = String.fromCharCode(b[i + 4], b[i + 5], b[i + 6], b[i + 7])
        const end = i + 12 + len
        if (end > b.length) return b
        if (!PNG_DROP.has(type)) parts.push(b.subarray(i, end))
        i = end
        if (type === 'IEND') break
    }
    return concat(parts)
}

const WEBP_DROP = new Set(['EXIF', 'XMP '])

/** A WebP without its EXIF and XMP chunks; the RIFF size and the VP8X flags are fixed to match. Damaged input comes back unchanged. */
export function stripWebpMetadata(b: Uint8Array): Uint8Array {
    if (sniffMime(b) !== 'image/webp') return b
    const view = new DataView(b.buffer, b.byteOffset, b.byteLength)
    const riffEnd = Math.min(b.length, 8 + view.getUint32(4, true))
    const parts: Uint8Array[] = []
    let dropped = false
    let i = 12
    while (i + 8 <= riffEnd) {
        const type = String.fromCharCode(b[i], b[i + 1], b[i + 2], b[i + 3])
        const len = view.getUint32(i + 4, true)
        const end = i + 8 + len + (len & 1)
        if (i + 8 + len > riffEnd) return b
        if (WEBP_DROP.has(type)) dropped = true
        else {
            const chunk = b.slice(i, Math.min(end, riffEnd))
            if (type === 'VP8X' && len >= 10) chunk[8] &= ~0x0c // the flags that announce EXIF (0x08) and XMP (0x04)
            parts.push(chunk)
        }
        i = end
    }
    if (!dropped) return b
    const body = concat(parts)
    const out = new Uint8Array(12 + body.length)
    out.set(b.subarray(0, 12))
    out.set(body, 12)
    new DataView(out.buffer).setUint32(4, out.length - 8, true)
    return out
}

/** A GIF without comment extensions and without application extensions other than the animation loop count (XMP lives in one). Damaged input comes back unchanged. */
export function stripGifMetadata(b: Uint8Array): Uint8Array {
    if (sniffMime(b) !== 'image/gif' || b.length < 13) return b
    const parts: Uint8Array[] = []
    const skipSubBlocks = (at: number): number => {
        while (at < b.length && b[at] !== 0) at += b[at] + 1
        return at + 1 // the terminator
    }
    let i = 13 + (b[10] & 0x80 ? 3 * 2 ** ((b[10] & 7) + 1) : 0)
    parts.push(b.subarray(0, i))
    let dropped = false
    while (i < b.length) {
        const intro = b[i]
        if (intro === 0x3b) {
            parts.push(b.subarray(i, i + 1))
            i = b.length
            break
        }
        if (intro === 0x21) {
            const label = b[i + 1]
            const end = label === 0xff || label === 0xfe || label === 0xf9 || label === 0x01 ? skipSubBlocks(i + 2) : -1
            if (end < 0 || end > b.length) return b
            const app = label === 0xff ? String.fromCharCode(...b.subarray(i + 3, i + 3 + 11)) : ''
            const keep = label === 0xf9 || label === 0x01 || (label === 0xff && (app === 'NETSCAPE2.0' || app === 'ANIMEXTS1.0'))
            if (keep) parts.push(b.subarray(i, end))
            else dropped = true
            i = end
        } else if (intro === 0x2c) {
            if (i + 10 > b.length) return b
            const lct = b[i + 9] & 0x80 ? 3 * 2 ** ((b[i + 9] & 7) + 1) : 0
            const end = skipSubBlocks(i + 10 + lct + 1)
            if (end > b.length) return b
            parts.push(b.subarray(i, end))
            i = end
        } else {
            return b
        }
    }
    return dropped ? concat(parts) : b
}

function concat(parts: Uint8Array[]): Uint8Array {
    const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0))
    let at = 0
    for (const p of parts) {
        out.set(p, at)
        at += p.length
    }
    return out
}

export function toBase64(b: Uint8Array): string {
    let s = ''
    for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode(...b.subarray(i, i + 0x8000))
    return btoa(s)
}

export const mimeAfterReencode: MediaMime = 'image/webp'

/** The browser's way to re-encode: decode, draw (at most 2048 px on a side), encode WebP at 0.7. */
export async function canvasReencode(bytes: Uint8Array, mime: MediaMime): Promise<Uint8Array | null> {
    try {
        if (typeof createImageBitmap !== 'function' || typeof OffscreenCanvas === 'undefined') return null
        const bitmap = await createImageBitmap(new Blob([bytes as BlobPart], { type: mime }))
        const scale = Math.min(1, 2048 / Math.max(bitmap.width, bitmap.height))
        const canvas = new OffscreenCanvas(Math.max(1, Math.round(bitmap.width * scale)), Math.max(1, Math.round(bitmap.height * scale)))
        canvas.getContext('2d')?.drawImage(bitmap, 0, 0, canvas.width, canvas.height)
        const blob = await canvas.convertToBlob({ type: 'image/webp', quality: 0.7 })
        return blob.type === 'image/webp' ? new Uint8Array(await blob.arrayBuffer()) : null
    } catch {
        return null
    }
}

export const defaultMediaDeps: MediaDeps = {
    async fetchFile(file) {
        try {
            const res = await transport.fetchFile(`/files/${encodeURIComponent(file.file_id)}`)
            if (!res.ok) return null
            return { bytes: new Uint8Array(await res.arrayBuffer()), mime: res.headers.get('content-type') ?? undefined }
        } catch {
            return null
        }
    },
    reencode: canvasReencode,
}

/** SHA-256 in hex; where `crypto.subtle` is missing, a length and two 32-bit sums (the hit is then checked byte by byte). */
export async function contentHash(bytes: Uint8Array): Promise<string> {
    const subtle = globalThis.crypto?.subtle
    if (subtle) {
        const digest = new Uint8Array(await subtle.digest('SHA-256', bytes as BufferSource))
        return Array.from(digest, (x) => x.toString(16).padStart(2, '0')).join('')
    }
    let a = 0x811c9dc5
    let b = 0
    for (let i = 0; i < bytes.length; i++) {
        a = Math.imul(a ^ bytes[i], 0x01000193) >>> 0
        b = (Math.imul(b, 31) + bytes[i]) >>> 0
    }
    return `${bytes.length}:${a.toString(16)}:${b.toString(16)}`
}

const sameBytes = (x: Uint8Array, y: Uint8Array) => x.length === y.length && x.every((v, i) => v === y[i])

/**
 * Builds the `media` map. An image is keyed by its CONTENT, not by the name it was given: two
 * pasted `image.png` with different pictures stay two entries, and one picture sent twice under two
 * names is stored once.
 */
export class MediaBuilder {
    readonly media: Record<string, MediaEntry> = {}
    /** Files that did not make it, once each, with why. */
    readonly left: { name: string; reason: PlaceholderReason; label: string }[] = []
    /** Outcome per picture content (hash -> candidates, compared byte for byte). */
    private readonly byContent = new Map<string, { bytes: Uint8Array; outcome: MediaOutcome }[]>()
    /** What a bare name resolved to in the session's files (a spec names files, it does not carry bytes). */
    private readonly byName = new Map<string, MediaOutcome>()
    private readonly goneNames: string[] = []

    constructor(
        private readonly files: readonly SessionFileRecord[],
        private readonly deps: MediaDeps = defaultMediaDeps
    ) {}

    async add(name: string, inline?: Uint8Array): Promise<MediaOutcome> {
        if (!inline) {
            const known = this.byName.get(name)
            if (known) return known
        }
        const outcome = await this.resolve(name, inline)
        if (!inline) this.byName.set(name, outcome)
        return outcome
    }

    /** The key a spec should use for `name`: the media key, or a key with no media behind it (the viewer shows "missing"). */
    async keyFor(name: string): Promise<string> {
        const o = await this.add(name)
        if (o.ok) return o.key
        let at = this.goneNames.indexOf(name)
        if (at < 0) at = this.goneNames.push(name) - 1
        return `gone-${at + 1}`
    }

    private fail(reason: PlaceholderReason, name: string): MediaOutcome {
        const what = reason === 'missing' ? 'is no longer available' : reason === 'oversized' ? 'is too large to include' : 'is not an image that can be shared'
        return { ok: false, reason, label: `${name.slice(0, 120)} ${what}` }
    }

    private async resolve(name: string, inline?: Uint8Array): Promise<MediaOutcome> {
        let bytes = inline
        if (!bytes) {
            const record = findFile(this.files, name)
            if (!record) return this.leave(name, this.fail('missing', name))
            const got = await this.deps.fetchFile(record)
            if (!got) return this.leave(name, this.fail('missing', name))
            bytes = got.bytes
        }
        const hash = await contentHash(bytes)
        const seen = this.byContent.get(hash)?.find((c) => sameBytes(c.bytes, bytes))
        // The same picture again: its entry, or its placeholder with this file's own name on it.
        if (seen) return seen.outcome.ok ? seen.outcome : this.leave(name, this.fail(seen.outcome.reason, name))
        const outcome = await this.build(name, bytes)
        const list = this.byContent.get(hash) ?? []
        list.push({ bytes, outcome })
        this.byContent.set(hash, list)
        return outcome.ok ? outcome : this.leave(name, outcome)
    }

    private leave(name: string, outcome: MediaOutcome): MediaOutcome {
        if (!outcome.ok && !this.left.some((l) => l.name === name && l.reason === outcome.reason)) this.left.push({ name, reason: outcome.reason, label: outcome.label })
        return outcome
    }

    private async build(name: string, original: Uint8Array): Promise<MediaOutcome> {
        let bytes = original
        let mime = sniffMime(bytes)
        if (!mime || !isAllowedMime(mime)) return this.fail('unsupported', name)
        // Whatever the size, the metadata goes: location, camera, authoring tool, comments.
        if (mime === 'image/jpeg') bytes = stripJpegMetadata(bytes)
        else if (mime === 'image/png') bytes = stripPngMetadata(bytes)
        else if (mime === 'image/webp') bytes = stripWebpMetadata(bytes)
        else if (mime === 'image/gif') bytes = stripGifMetadata(bytes)
        if (bytes.length > SHARE_LIMITS.mediaBytes) {
            const again = this.deps.reencode ? await this.deps.reencode(bytes, mime) : null
            if (!again || again.length > SHARE_LIMITS.mediaBytes || sniffMime(again) !== 'image/webp') return this.fail('oversized', name)
            bytes = stripWebpMetadata(again)
            mime = 'image/webp'
        }
        if (Object.keys(this.media).length >= SHARE_LIMITS.mediaItems) return this.fail('oversized', name)
        const key = `m${Object.keys(this.media).length + 1}`
        this.media[key] = { mime, bytes: bytes.length, data: toBase64(bytes) }
        return { ok: true, key }
    }
}

export const ALLOWED = MEDIA_MIME_ALLOW

/** Every file name a spec refers to, once each. */
export function specFileNames(spec: unknown): string[] {
    const elements = (spec as { elements?: unknown } | null)?.elements
    if (!elements || typeof elements !== 'object') return []
    try {
        return [...new Set(fileReferences(elements as Parameters<typeof fileReferences>[0]).map((r) => r.name))]
    } catch {
        return []
    }
}

/** A copy of `spec` with each referenced file name replaced by `keys.get(name)` (names without a key are left as they are). */
export function rewriteSpecFiles<T>(spec: T, keys: ReadonlyMap<string, string>): T {
    const copy = structuredClone(spec) as { elements?: Record<string, { type?: string; props?: Record<string, unknown> }> }
    const swap = (v: unknown) => (typeof v === 'string' && keys.has(v) ? (keys.get(v) as string) : v)
    for (const element of Object.values(copy?.elements ?? {})) {
        const props = element?.props
        if (!props || typeof props !== 'object') continue
        for (const prop of ['file', 'poster']) if (typeof props[prop] === 'string') props[prop] = swap(props[prop])
        if (Array.isArray(props.files)) props.files = props.files.map(swap)
        if (element.type === 'Canvas2D' && Array.isArray(props.shapes)) {
            for (const shape of props.shapes) {
                if (shape && typeof shape === 'object' && (shape as { kind?: string }).kind === 'image') (shape as { file?: unknown }).file = swap((shape as { file?: unknown }).file)
            }
        }
    }
    return copy as T
}
