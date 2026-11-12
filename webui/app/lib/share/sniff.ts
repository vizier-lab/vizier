// The type of a raster image from its first bytes. Pure and tiny: the viewer bundle uses it too,
// so it must not pull in the API client that `media.ts` needs.
import type { MediaMime } from './limits'

export function sniffMime(b: Uint8Array): MediaMime | null {
    if (b.length >= 8 && b[0] === 0x89 && b[1] === 0x50 && b[2] === 0x4e && b[3] === 0x47) return 'image/png'
    if (b.length >= 3 && b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff) return 'image/jpeg'
    if (b.length >= 6 && b[0] === 0x47 && b[1] === 0x49 && b[2] === 0x46 && b[3] === 0x38) return 'image/gif'
    if (b.length >= 12 && b[0] === 0x52 && b[1] === 0x49 && b[2] === 0x46 && b[3] === 0x46 && b[8] === 0x57 && b[9] === 0x45 && b[10] === 0x42 && b[11] === 0x50) return 'image/webp'
    return null
}

/** The type claimed by `mime` must be the type the bytes are. Reads only the first 16 bytes (24 base64 characters). */
export function magicMatches(mime: string, base64: string): boolean {
    try {
        const head = base64.slice(0, 24)
        const bin = atob(head.padEnd(Math.ceil(head.length / 4) * 4, '='))
        return sniffMime(Uint8Array.from(bin, (c) => c.charCodeAt(0))) === mime
    } catch {
        return false
    }
}
