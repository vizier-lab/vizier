// The crash-loop marker (specs/063-html-generative-ui FR-016a, tasks T045–T046).
//
// Where a page's frame shares the WebUI's thread (WebKit, and Chromium when it keeps the frame in
// process), a page that loops forever freezes the whole tab, and a reload would run it again. So a
// version is marked in `localStorage` before it mounts and unmarked once it has beaten three
// times (it runs) or when it unmounts or the page is left normally. A marker still there when the
// WebUI loads means that version froze the last tab: it is drawn paused until the person runs it
// anyway, and the agent's next turn in that conversation is told.
//
// Storage can be missing or throw (private windows, blocked site data); then nothing is paused.

const KEY = 'vz.mounting'
/** Heartbeats after which a mounted version counts as running. */
export const BEATS_TO_CLEAR = 3
/** At most this many paused pages are told to the agent on one request. */
const MAX_NOTES = 3

export type Mounting = { page: string; version: number; topic: string; at: number }

const id = (page: string, version: number) => `${page}:${version}`

function read(): Record<string, Mounting> {
    try {
        const raw = localStorage.getItem(KEY)
        const value = raw ? (JSON.parse(raw) as unknown) : null
        return value && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, Mounting>) : {}
    } catch {
        return {}
    }
}

function write(all: Record<string, Mounting>) {
    try {
        if (Object.keys(all).length) localStorage.setItem(KEY, JSON.stringify(all))
        else localStorage.removeItem(KEY)
    } catch {
        // Nothing to keep: the marker is a convenience.
    }
}

/** What a previous life of the WebUI left marked: the versions that froze it. */
const pausedAtLoad: Record<string, Mounting> = typeof window === 'undefined' ? {} : read()
/** What this life marked and has not cleared yet. */
const mine = new Set<string>()
const listeners = new Set<() => void>()

if (typeof window !== 'undefined') {
    // Leaving normally is not a freeze: a frozen tab never gets here.
    window.addEventListener('pagehide', () => {
        const all = read()
        for (const key of mine) delete all[key]
        write(all)
    })
}

/** Whether this version froze the tab last time and has not been run anyway since. */
export function isPaused(page: string, version: number): boolean {
    return id(page, version) in pausedAtLoad
}

/** The person chose to run a paused version: it is no longer paused (it is marked again on mount). */
export function runAnyway(page: string, version: number) {
    const key = id(page, version)
    delete pausedAtLoad[key]
    const all = read()
    delete all[key]
    write(all)
    for (const fn of listeners) fn()
}

export function onPausedChange(fn: () => void): () => void {
    listeners.add(fn)
    return () => listeners.delete(fn)
}

/** Mark a version as mounting, before its frame loads. */
export function markMounting(page: string, version: number, topic: string) {
    const key = id(page, version)
    mine.add(key)
    write({ ...read(), [key]: { page, version, topic, at: Date.now() } })
}

/** The version runs (or is gone): unmark it. */
export function clearMounting(page: string, version: number) {
    const key = id(page, version)
    if (!mine.delete(key)) return
    const all = read()
    delete all[key]
    write(all)
}

/**
 * The client-context notes for a request in conversation `topic` (a topic key): one line per
 * version of that conversation that froze the tab and is still paused.
 */
export function pausedNotes(topic: string): string[] {
    return Object.values(pausedAtLoad)
        .filter((m) => m.topic === topic)
        .slice(0, MAX_NOTES)
        .map(
            (m) =>
                `Page ${m.page} v${m.version} froze the person's tab last time it ran and is paused. Move its heavy work into vizier.worker (see the ui_render guide) before drawing it again.`
        )
}
