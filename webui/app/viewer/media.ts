// Images of a shared copy come from the snapshot's `media` map and nowhere else: a `data:` address
// built here from a raster MIME on the allow-list and base64 text, never a remote URL.
import type { SessionFileRecord } from '../api/frames'
import { isAllowedMime } from '../lib/share/limits'
import type { MediaEntry, Snapshot } from '../lib/share/schema'

const BASE64 = /^[A-Za-z0-9+/]*={0,2}$/

/** The address for one media entry, or null when its type or data is not acceptable. */
export function mediaSrc(entry: MediaEntry | undefined): string | null {
    if (!entry || !isAllowedMime(entry.mime) || typeof entry.data !== 'string' || !BASE64.test(entry.data)) return null
    return `data:${entry.mime};base64,${entry.data}`
}

/** The media map as the file list a drawn interface looks its images up in (name = media key). */
export function mediaFiles(media: Snapshot['media']): SessionFileRecord[] {
    const out: SessionFileRecord[] = []
    for (const [key, entry] of Object.entries(media)) {
        const src = mediaSrc(entry)
        if (src) out.push({ id: key, session_slug: '', agent_id: '', filename: key, mime_type: entry.mime, size: entry.bytes, file_id: src, added_at: '' })
    }
    return out
}
