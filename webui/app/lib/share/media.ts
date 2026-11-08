// Images of a conversation, made safe to embed (research R3): raster allow-list by content (not by the
// name or the claimed type), metadata dropped, big images re-encoded once, anything else a labelled
// placeholder. The output is the snapshot's `media` map and the name -> key rewrite for specs.
import type { SessionFileRecord } from '../../api/frames'
import { transport } from '../../api/transport'
import { fileReferences, findFile } from '../generativeUi/media'
import { isAllowedMime, MEDIA_MIME_ALLOW, SHARE_LIMITS, type MediaMime } from './limits'
import type { MediaEntry, PlaceholderReason } from './schema'

export interface FetchedFile { bytes: Uint8Array; mime?: string }
export interface MediaDeps {
    /** The bytes of a stored file, or null when it cannot be read. */
    fetchFile(file: SessionFileRecord): Promise<FetchedFile | null>
    /** One re-encode to WebP at lower quality; null when the platform cannot. */
    reencode?(bytes: Uint8Array, mime: MediaMime): Promise<Uint8Array | null>
}

export type MediaOutcome = { ok: true; key: string } | { ok: false; reason: PlaceholderReason; label: string }

/** The type of an image from its first bytes; anything not in the allow-list (SVG, HTML, audio) is null. */
export function sniffMime(b: Uint8Array): MediaMime | null {
    if (b.length >= 8 && b[0] === 0x89 && b[1] === 0x50 && b[2] === 0x4e && b[3] === 0x47) return 'image/png'
    if (b.length >= 3 && b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff) return 'image/jpeg'
    if (b.length >= 6 && b[0] === 0x47 && b[1] === 0x49 && b[2] === 0x46 && b[3] === 0x38) return 'image/gif'
    if (b.length >= 12 && b[0] === 0x52 && b[1] === 0x49 && b[2] === 0x46 && b[3] === 0x46 && b[8] === 0x57 && b[9] === 0x45 && b[10] === 0x42 && b[11] === 0x50) return 'image/webp'
    return null
}

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

/** Builds the `media` map one file name at a time; a name is fetched once. */
export class MediaBuilder {
    readonly media: Record<string, MediaEntry> = {}
    /** Names that did not make it, once each, with why. */
    readonly left: { name: string; reason: PlaceholderReason; label: string }[] = []
    private readonly byName = new Map<string, MediaOutcome>()
    private gone = 0

    constructor(
        private readonly files: readonly SessionFileRecord[],
        private readonly deps: MediaDeps = defaultMediaDeps
    ) {}

    async add(name: string, inline?: Uint8Array): Promise<MediaOutcome> {
        const known = this.byName.get(name)
        if (known) return known
        const outcome = await this.build(name, inline)
        this.byName.set(name, outcome)
        if (!outcome.ok) this.left.push({ name, reason: outcome.reason, label: outcome.label })
        return outcome
    }

    /** The key a spec should use for `name`: the media key, or a key with no media behind it (the viewer shows "missing"). */
    async keyFor(name: string): Promise<string> {
        const o = await this.add(name)
        return o.ok ? o.key : `gone-${[...this.byName.keys()].indexOf(name) + 1}`
    }

    private fail(reason: PlaceholderReason, name: string): MediaOutcome {
        const what = reason === 'missing' ? 'is no longer available' : reason === 'oversized' ? 'is too large to include' : 'is not an image that can be shared'
        return { ok: false, reason, label: `${name.slice(0, 120)} ${what}` }
    }

    private async build(name: string, inline?: Uint8Array): Promise<MediaOutcome> {
        let bytes = inline
        if (!bytes) {
            const record = findFile(this.files, name)
            if (!record) return this.fail('missing', name)
            const got = await this.deps.fetchFile(record)
            if (!got) return this.fail('missing', name)
            bytes = got.bytes
        }
        let mime = sniffMime(bytes)
        if (!mime || !isAllowedMime(mime)) return this.fail('unsupported', name)
        if (mime === 'image/jpeg') bytes = stripJpegMetadata(bytes)
        else if (mime === 'image/png') bytes = stripPngMetadata(bytes)
        if (bytes.length > SHARE_LIMITS.mediaBytes) {
            const again = this.deps.reencode ? await this.deps.reencode(bytes, mime) : null
            if (!again || again.length > SHARE_LIMITS.mediaBytes || sniffMime(again) !== 'image/webp') return this.fail('oversized', name)
            bytes = again
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
