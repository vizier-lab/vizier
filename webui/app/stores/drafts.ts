// Unsaved text per document, kept in this browser so a reload or a stray click loses nothing.
// Keys: `core:<agent>`, `skill:<agent>:<slug>`, `memory:<agent>:<bundle>/<path>`.

const PREFIX = 'vizier.draft.'

type Draft = { text: string; base: string; at: number }

function read(key: string): Draft | null {
    try {
        const raw = localStorage.getItem(PREFIX + key)
        return raw ? (JSON.parse(raw) as Draft) : null
    } catch {
        return null
    }
}

/** Saves what the person has typed, with the text it started from. A draft equal to its base is dropped. */
export function saveDraft(key: string, text: string, base: string) {
    try {
        if (text === base) localStorage.removeItem(PREFIX + key)
        else localStorage.setItem(PREFIX + key, JSON.stringify({ text, base, at: Date.now() } satisfies Draft))
    } catch {
        /* storage blocked or full: the draft just isn't kept */
    }
}

export function clearDraft(key: string) {
    try {
        localStorage.removeItem(PREFIX + key)
    } catch {
        /* nothing to clear */
    }
}

/**
 * A draft worth offering back: it differs from what is saved now. `stale` is true when the saved
 * text has changed since the draft began (someone else saved), so restoring it would overwrite.
 */
export function pendingDraft(key: string, saved: string): { text: string; at: number; stale: boolean } | null {
    const d = read(key)
    if (!d || d.text === saved) return null
    return { text: d.text, at: d.at, stale: d.base !== saved }
}
