// Every limit of the shared-conversation feature in one place (specs/056-shared-chat research R12).
// The Rust twin is the consts at the top of `crates/vizier-core/src/service/shares.rs`; keep them equal.
import { LIMITS } from '../generativeUi/limits'

export const SHARE_LIMITS = {
    /** The whole POST body, and the stored snapshot text. */
    bodyBytes: 5 * 1024 * 1024,
    /** Room the request wrapper (ids, title, people, removals) may take beside the snapshot. */
    envelopeBytes: 16 * 1024,
    /** One media item, raw bytes after re-encoding. */
    mediaBytes: 1024 * 1024,
    /** Media items in one snapshot (equals the generative UI's distinct-file limit). */
    mediaItems: LIMITS.distinctFiles,
    titleLength: 120,
    people: 50,
    turns: 5000,
    parts: 64,
    /** One text part, in UTF-8 bytes. */
    textBytes: 256 * 1024,
    /** A `ui` part's spec, in bytes of JSON. */
    specBytes: LIMITS.specBytes,
    /** JSON nesting allowed in a spec, state or tool arguments read from a snapshot. */
    jsonDepth: 64,
    /** A tool result kept in the trail is cut to this many characters. */
    trailResultChars: 20_000,
    /** Tool arguments kept in the trail, bytes of JSON; more is replaced by a note. */
    trailArgumentBytes: 32 * 1024,
    createPerHour: 10,
    activePerPerson: 200,
    /** Failed opens of a share per visitor per minute (FR-049). */
    missPerMinute: 60,
    /** People search: most matches per search, searches per minute per person (FR-012). */
    peopleResults: 10,
    peopleSearchesPerMinute: 30,
} as const

/** Raster formats a snapshot may carry. SVG and everything else can hold script or be too big to vouch for. */
export const MEDIA_MIME_ALLOW = ['image/png', 'image/jpeg', 'image/webp', 'image/gif'] as const
export type MediaMime = (typeof MEDIA_MIME_ALLOW)[number]
export const isAllowedMime = (mime: string): mime is MediaMime => (MEDIA_MIME_ALLOW as readonly string[]).includes(mime)

export const SNAPSHOT_FORMAT_VERSION = 1
