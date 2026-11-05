import { create } from 'zustand'
import {
    listNotifications,
    markAllNotificationsRead,
    notificationCounts,
    setNotificationRead,
    type NotificationCounts,
    type NotificationFilter,
    type NotificationItem,
} from '../api/endpoints/notifications'
import { transport, type ConnectionState } from '../api/transport'
import { absorb, onHello, onRead, parseFrame, type InboxFrame } from '../lib/inboxFrames'
import { applyAllRead, applyRead, belongs, mergePage } from '../lib/notifications'
import { useAgents } from './agents'

const PAGE = 20

const EMPTY: NotificationCounts = { all: 0, unread: 0, read: 0 }

type NotificationStore = {
    /** Whether the dropdown is showing. */
    open: boolean
    filter: NotificationFilter
    items: NotificationItem[]
    counts: NotificationCounts
    /** The cursor for the next page; null on the last one. */
    next: string | null
    loading: boolean
    loadingMore: boolean
    loaded: boolean
    error: string | null
    /** The highest id seen, so a later one is new. */
    lastSeenId: number
    /** Whether the first snapshot has established `lastSeenId` (its rows are not new). */
    primed: boolean
    /** Rows that arrived since the toaster last drained them. */
    arrived: NotificationItem[]
    takeArrived: () => NotificationItem[]
    /** The in-app path on screen, so a reply to that conversation is read on arrival. */
    viewing: string | null
    setViewing: (path: string) => void
    openPanel: () => void
    closePanel: () => void
    setFilter: (filter: NotificationFilter) => void
    /** The first page of the current filter, replacing what is loaded. */
    load: () => Promise<void>
    /** A silent first-page merge, for when the panel opens on a list already loaded. */
    refresh: () => Promise<void>
    loadMore: () => Promise<void>
    markRead: (id: number, read: boolean) => Promise<void>
    markAll: () => Promise<void>
    /** Apply one frame of the inbox stream. Also the seam the tests drive. */
    receive: (frame: InboxFrame) => void
    /**
     * Re-read the newest rows after a gap (a reconnect that missed some, or a `resync`): rows above
     * the high-water mark are new, and the loaded first page is replaced so rows that went away
     * (a deleted conversation) and read flips made elsewhere are right.
     */
    catchUp: () => Promise<void>
    /**
     * Opens the inbox stream: counts feed the badge, ids above the high-water mark are new (for a
     * toast), and setup progress goes to the agents store. No timer. Returns a stop function.
     */
    start: () => () => void
    /** Sign-out or a fresh mount: the previous person's counts must not flash. */
    reset: () => void
}

/** The counts after one row changes, so the badge moves before the server answers. */
function optimisticCounts(
    counts: NotificationCounts,
    was: boolean,
    now: boolean
): NotificationCounts {
    if (was === now) return counts
    return now
        ? { ...counts, unread: Math.max(0, counts.unread - 1), read: counts.read + 1 }
        : { ...counts, unread: counts.unread + 1, read: Math.max(0, counts.read - 1) }
}

export const useNotifications = create<NotificationStore>()((set, get) => ({
    open: false,
    filter: 'unread',
    items: [],
    counts: EMPTY,
    next: null,
    loading: false,
    loadingMore: false,
    loaded: false,
    error: null,
    lastSeenId: 0,
    primed: false,
    arrived: [],
    viewing: null,

    setViewing: (path) => set({ viewing: path }),

    takeArrived: () => {
        const items = get().arrived
        if (items.length) set({ arrived: [] })
        return items
    },

    openPanel: () => {
        set({ open: true })
        if (get().loaded) void get().refresh()
        else void get().load()
    },

    closePanel: () => set({ open: false }),

    setFilter: (filter) => {
        set({ filter, items: [], next: null, loaded: false, error: null })
        void get().load()
    },

    load: async () => {
        const { filter } = get()
        set({ loading: true, error: null })
        try {
            const page = await listNotifications(filter, undefined, PAGE)
            // A slower request for an old filter must not overwrite a newer one.
            if (get().filter !== filter) return
            set({
                items: page.items,
                counts: page.counts,
                next: page.next ?? null,
                loading: false,
                loaded: true,
            })
        } catch {
            set({ loading: false, error: "Couldn't load notifications" })
        }
    },

    refresh: async () => {
        const { filter } = get()
        try {
            const page = await listNotifications(filter, undefined, PAGE)
            if (get().filter !== filter) return
            const merged = mergePage(get().items, page.items, filter)
            set((state) => ({
                items: merged,
                counts: page.counts,
                next: state.next ?? page.next ?? null,
                loaded: true,
                error: null,
            }))
        } catch {
            // Keep the last known list and counts: a failed poll is not an empty inbox.
        }
    },

    loadMore: async () => {
        const { next, loadingMore, filter, items } = get()
        if (!next || loadingMore) return
        set({ loadingMore: true })
        try {
            const page = await listNotifications(filter, next, PAGE)
            if (get().filter !== filter) return
            const known = new Set(items.map((i) => i.id))
            const added = page.items.filter((i) => !known.has(i.id))
            set({
                items: [...items, ...added],
                counts: page.counts,
                next: page.next ?? null,
                loadingMore: false,
            })
        } catch {
            set({ loadingMore: false })
        }
    },

    markRead: async (id, read) => {
        const { filter, items, counts } = get()
        const current = items.find((i) => i.id === id)
        const before = { items, counts }
        set({
            items: applyRead(items, id, read, filter),
            counts: current ? optimisticCounts(counts, current.read, read) : counts,
        })
        try {
            const server = await setNotificationRead(id, read)
            set({ counts: server })
        } catch {
            set(before)
        }
    },

    markAll: async () => {
        const { filter, items, counts } = get()
        const before = { items, counts }
        set({
            items: applyAllRead(items, filter),
            counts: { ...counts, unread: 0, read: counts.all },
        })
        try {
            const server = await markAllNotificationsRead()
            set({ counts: server })
        } catch {
            set(before)
        }
    },

    receive: (frame) => {
        const s = get()
        switch (frame.type) {
            case 'hello': {
                const { patch, fetchPage } = onHello(s, frame)
                set(patch)
                useAgents.getState().replaceSetups(frame.setups)
                if (fetchPage) void get().catchUp()
                break
            }
            case 'notification': {
                const { patch, markRead } = absorb(s, [frame.item], frame.counts)
                set(patch)
                for (const id of markRead) void setNotificationRead(id, true).catch(() => {})
                break
            }
            case 'read':
                set(onRead(s, frame.id, frame.read, frame.counts))
                break
            case 'setup':
                useAgents.getState().applySetup(frame.state)
                break
            case 'setup_cleared':
                useAgents.getState().clearSetup(frame.agent_id)
                break
            case 'resync':
                void get().catchUp()
                break
        }
    },

    catchUp: async () => {
        let page
        try {
            page = await listNotifications('all', undefined, PAGE)
        } catch {
            return // keep what is held: a failed read is not an empty inbox
        }
        const { patch, markRead } = absorb(get(), page.items, page.counts)
        set(patch)
        for (const id of markRead) void setNotificationRead(id, true).catch(() => {})
        if (!get().loaded) return
        const filter = get().filter
        try {
            const first = filter === 'all' ? page : await listNotifications(filter, undefined, PAGE)
            if (get().filter !== filter) return
            const read = new Set(markRead)
            set({
                items: first.items
                    .map((i) => (read.has(i.id) ? { ...i, read: true } : i))
                    .filter((i) => belongs(filter, i)),
                next: first.next ?? null,
                error: null,
            })
        } catch {
            /* keep the list as it was */
        }
    },

    start: () => {
        const handle = transport.openInbox({
            onFrame: (data) => {
                const frame = parseFrame(data)
                if (frame) get().receive(frame)
            },
            onOpen: () => {},
            onState: (state: ConnectionState) => {
                // A refused upgrade is invisible to the browser, so a stream that keeps failing
                // asks one ordinary question: a 401 there signs the person out (`settle`), and a
                // live answer corrects the badge meanwhile.
                if (state === 'offline') {
                    void notificationCounts()
                        .then((counts) => counts && set({ counts }))
                        .catch(() => {})
                }
            },
        })
        return () => handle.close()
    },

    reset: () => {
        set({
            open: false,
            filter: 'unread',
            items: [],
            counts: EMPTY,
            next: null,
            loading: false,
            loadingMore: false,
            loaded: false,
            error: null,
            lastSeenId: 0,
            primed: false,
            arrived: [],
            viewing: null,
        })
        // The previous person's setup progress must not flash either.
        useAgents.getState().resetSetups()
    },
}))
