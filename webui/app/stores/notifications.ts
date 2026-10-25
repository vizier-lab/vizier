import { create } from 'zustand'
import {
    listNotifications,
    markAllNotificationsRead,
    setNotificationRead,
    type NotificationCounts,
    type NotificationFilter,
    type NotificationItem,
} from '../api/endpoints/notifications'
import { applyAllRead, applyRead, isViewing, mergePage } from '../lib/notifications'

/** Within the spec's 30s freshness budget. */
const POLL_MS = 15_000
const PAGE = 20

const EMPTY: NotificationCounts = { all: 0, unread: 0, read: 0 }

type NotificationStore = {
    /** Whether the slide-over is showing. */
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
    /** The highest id the poll has seen, so a later one is new. */
    lastSeenId: number
    /** Whether the first poll has established `lastSeenId` (its rows are not new). */
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
    /** A silent first-page merge, for a poll while the panel is open. */
    refresh: () => Promise<void>
    loadMore: () => Promise<void>
    markRead: (id: number, read: boolean) => Promise<void>
    markAll: () => Promise<void>
    /**
     * Polls the newest page every 15s while the tab is visible: its counts feed the badge,
     * and ids above the high-water mark are new (for a toast). Returns a stop function.
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
    filter: 'all',
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

    start: () => {
        const tick = async () => {
            if (typeof document !== 'undefined' && document.visibilityState !== 'visible') return
            // One request answers both questions: the badge (its counts) and what is new
            // (ids above the high-water mark). A failure keeps the last known badge.
            let page
            try {
                page = await listNotifications('all', undefined, PAGE)
            } catch {
                return
            }
            const state = get()
            let counts = page.counts
            let lastSeenId = state.lastSeenId
            let arrived = state.arrived
            let readNow: number[] = []

            if (!state.primed) {
                lastSeenId = page.items.reduce((max, i) => Math.max(max, i.id), 0)
            } else {
                const fresh = page.items.filter((i) => i.id > state.lastSeenId)
                if (fresh.length) {
                    const viewed = fresh.filter((i) => isViewing(i, state.viewing))
                    const toastable = fresh.filter((i) => !isViewing(i, state.viewing))
                    lastSeenId = fresh.reduce((max, i) => Math.max(max, i.id), state.lastSeenId)
                    if (viewed.length) {
                        // The reply is already on screen: read it with no badge bump and no toast.
                        counts = {
                            ...counts,
                            unread: Math.max(0, counts.unread - viewed.length),
                            read: counts.read + viewed.length,
                        }
                        readNow = viewed.map((i) => i.id)
                        for (const item of viewed) {
                            void setNotificationRead(item.id, true).catch(() => {})
                        }
                    }
                    // With the panel open the new row is already at its top.
                    if (!state.open) arrived = [...state.arrived, ...toastable]
                }
            }
            set({ counts, lastSeenId, primed: true, arrived })
            if (readNow.length) {
                const ids = new Set(readNow)
                set((s) => ({
                    items: s.items.map((i) => (ids.has(i.id) ? { ...i, read: true } : i)),
                }))
            }
            // Keep an open panel current; `all` can reuse the page just fetched.
            if (state.open && state.loaded) {
                if (state.filter === 'all') {
                    const ids = new Set(readNow)
                    set((s) => {
                        const merged = mergePage(s.items, page.items, 'all')
                        return {
                            items: ids.size
                                ? merged.map((i) => (ids.has(i.id) ? { ...i, read: true } : i))
                                : merged,
                            next: s.next ?? page.next ?? null,
                            error: null,
                        }
                    })
                } else {
                    await get().refresh()
                }
            }
        }
        void tick()
        const id = setInterval(() => void tick(), POLL_MS)
        return () => clearInterval(id)
    },

    reset: () =>
        set({
            open: false,
            filter: 'all',
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
        }),
}))
