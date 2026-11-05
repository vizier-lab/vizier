// What the inbox stream's frames do to the stores (`specs/051-realtime-notifications`
// contracts/inbox-stream.md). Pure: each function takes the slice of state it reads and returns
// the patch to apply, so the rules are tested without a socket and the stores stay thin.
import type { SetupState } from '../api/endpoints/agents'
import type { NotificationCounts, NotificationFilter, NotificationItem } from '../api/endpoints/notifications'
import { applyAllRead, applyRead, isViewing, mergePage } from './notifications'
import { SAMPLES, setupProgress, type SetupSample } from './setupProgress'

export type InboxFrame =
    | { type: 'hello'; counts: NotificationCounts; latest_id: number; setups: SetupState[] }
    | { type: 'notification'; item: NotificationItem; counts: NotificationCounts }
    | { type: 'read'; id?: number; read: boolean; counts: NotificationCounts }
    | { type: 'setup'; state: SetupState }
    | { type: 'setup_cleared'; agent_id: string }
    | { type: 'resync' }

const isObject = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object'

/** A frame this client understands, or null. Unknown types are ignored so frames can be added. */
export function parseFrame(data: unknown): InboxFrame | null {
    if (!isObject(data)) return null
    switch (data.type) {
        case 'hello':
            return isObject(data.counts) && Array.isArray(data.setups) ? (data as unknown as InboxFrame) : null
        case 'notification':
            return isObject(data.item) && isObject(data.counts) ? (data as unknown as InboxFrame) : null
        case 'read':
            return typeof data.read === 'boolean' && isObject(data.counts) ? (data as unknown as InboxFrame) : null
        case 'setup':
            return isObject(data.state) && typeof data.state.agent_id === 'string' ? (data as unknown as InboxFrame) : null
        case 'setup_cleared':
            return typeof data.agent_id === 'string' ? (data as unknown as InboxFrame) : null
        case 'resync':
            return { type: 'resync' }
        default:
            return null
    }
}

// ------------------------------------------------------------------------------ notifications

/** The part of the notifications store that frames read and change. */
export type InboxNotes = {
    open: boolean
    filter: NotificationFilter
    items: NotificationItem[]
    counts: NotificationCounts
    loaded: boolean
    /** The highest id seen; an id at or below it is never new. */
    lastSeenId: number
    /** Whether the first snapshot has set `lastSeenId` (its rows are not new). */
    primed: boolean
    arrived: NotificationItem[]
    /** The in-app path on screen, so a reply to that conversation is read on arrival. */
    viewing: string | null
}

export type Absorbed = {
    patch: Partial<InboxNotes>
    /** Rows to mark read on the server: replies to the conversation on screen. */
    markRead: number[]
}

/**
 * Take in rows the client may not have seen (a pushed row, or a page fetched after a gap).
 * Rows above the high-water mark are new: a reply to the conversation on screen is read with no
 * toast and no badge bump, anything else is queued for one toast per burst unless the panel is
 * open (the row is already at its top).
 */
export function absorb(s: InboxNotes, rows: NotificationItem[], counts: NotificationCounts): Absorbed {
    if (!s.primed) {
        const lastSeenId = rows.reduce((max, r) => Math.max(max, r.id), s.lastSeenId)
        return {
            patch: {
                counts,
                lastSeenId,
                primed: true,
                ...(s.loaded ? { items: mergePage(s.items, rows, s.filter) } : {}),
            },
            markRead: [],
        }
    }
    const fresh = rows.filter((r) => r.id > s.lastSeenId)
    let adjusted = counts
    let arrived = s.arrived
    let lastSeenId = s.lastSeenId
    let markRead: number[] = []
    if (fresh.length) {
        const viewed = fresh.filter((r) => isViewing(r, s.viewing))
        const toastable = fresh.filter((r) => !isViewing(r, s.viewing))
        lastSeenId = fresh.reduce((max, r) => Math.max(max, r.id), s.lastSeenId)
        if (viewed.length) {
            adjusted = {
                ...counts,
                unread: Math.max(0, counts.unread - viewed.length),
                read: counts.read + viewed.length,
            }
            markRead = viewed.map((r) => r.id)
        }
        if (!s.open) arrived = [...s.arrived, ...toastable]
    }
    const patch: Partial<InboxNotes> = { counts: adjusted, lastSeenId, primed: true, arrived }
    if (s.loaded) {
        const read = new Set(markRead)
        patch.items = mergePage(
            s.items,
            read.size ? rows.map((r) => (read.has(r.id) ? { ...r, read: true } : r)) : rows,
            s.filter
        )
    }
    return { patch, markRead }
}

/** This person's read state changed on any client. No `id` means everything. */
export function onRead(s: InboxNotes, id: number | undefined, read: boolean, counts: NotificationCounts): Partial<InboxNotes> {
    return {
        counts,
        items: id === undefined ? applyAllRead(s.items, s.filter) : applyRead(s.items, id, read, s.filter),
    }
}

/**
 * The snapshot a connection opens with. The first one only sets the high-water mark, since
 * what is already there is not news. A later one (after a drop) says whether rows arrived while
 * the client was away, and whether the loaded list should be re-read to catch read flips.
 */
export function onHello(
    s: InboxNotes,
    hello: { counts: NotificationCounts; latest_id: number }
): { patch: Partial<InboxNotes>; fetchPage: boolean } {
    if (!s.primed) {
        return {
            patch: { counts: hello.counts, lastSeenId: Math.max(s.lastSeenId, hello.latest_id), primed: true },
            fetchPage: false,
        }
    }
    return {
        patch: { counts: hello.counts },
        fetchPage: hello.latest_id > s.lastSeenId || s.loaded,
    }
}

// ----------------------------------------------------------------------------------- setups

/** The part of the agents store that setup frames read and change. */
export type InboxSetups = {
    setups: Record<string, SetupState>
    /** The last few progress samples per starting agent, for the ETA. */
    samples: Record<string, SetupSample[]>
}

/** A finished setup lingers this long, then the client drops it. */
export const TERMINAL_MS = 5000

export const isTerminal = (s: SetupState) => s.phase === 'ready' || s.phase === 'failed'

function sampled(taken: SetupSample[] | undefined, state: SetupState, now: number): SetupSample[] | undefined {
    const progress = setupProgress(state)
    if (progress == null) return taken
    return [...(taken ?? []), { at: now, progress }].slice(-SAMPLES)
}

/** One agent's setup changed. */
export function onSetup(s: InboxSetups, state: SetupState, now: number): InboxSetups {
    const taken = sampled(s.samples[state.agent_id], state, now)
    const samples = { ...s.samples }
    if (taken) samples[state.agent_id] = taken
    return { setups: { ...s.setups, [state.agent_id]: state }, samples }
}

/** The setup is gone. */
export function onSetupCleared(s: InboxSetups, agentId: string): InboxSetups {
    const setups = { ...s.setups }
    const samples = { ...s.samples }
    delete setups[agentId]
    delete samples[agentId]
    return { setups, samples }
}

/** A snapshot replaces every setup. Samples are kept only for agents still setting up. */
export function onHelloSetups(s: InboxSetups, list: SetupState[], now: number): InboxSetups {
    const live = new Set(list.map((x) => x.agent_id))
    let samples: Record<string, SetupSample[]> = {}
    for (const [id, taken] of Object.entries(s.samples)) if (live.has(id)) samples[id] = taken
    for (const state of list) {
        const taken = sampled(samples[state.agent_id], state, now)
        if (taken) samples = { ...samples, [state.agent_id]: taken }
    }
    return { setups: Object.fromEntries(list.map((x) => [x.agent_id, x])), samples }
}

/**
 * The patch that drops a finished setup, or null when nothing should change: the server only
 * prunes on a read, so the client removes a `ready` or `failed` entry itself after
 * [`TERMINAL_MS`]. `held` is the entry the timer was set for; a newer one for the same agent
 * (a restart) is left alone.
 */
export function dropTerminal(s: InboxSetups, held: SetupState): InboxSetups | null {
    if (s.setups[held.agent_id] !== held) return null
    return onSetupCleared(s, held.agent_id)
}
