import { API_BASE, relativeToApi, transport } from '../api/transport'

/**
 * Where the server keeps a file, as something an `<img>`, `<audio>` or `<video>` can load.
 *
 * The engine hands out a path under /api/v1/files (a `FileRecord.url`, an `avatar_url`). A browser
 * fetches that from its own origin; the desktop window's origin is the Tauri shell, which would
 * ask its bundled assets for the path and get nothing back, so the request goes through the
 * `vizier-asset` scheme instead. Remote URLs (`https://cdn…`) are not paths and are left alone.
 */
export function assetPath(path: string): string {
    return transport.authMode === 'desktop' ? transport.assetUrl(path) : `${API_BASE}${relativeToApi(path)}`
}

/** `assetPath`, passing a missing path through as `undefined`. */
export function assetSrc(path?: string | null): string | undefined {
    return path ? assetPath(path) : undefined
}
