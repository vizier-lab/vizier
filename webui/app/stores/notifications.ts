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
import { applyAllRead, applyRead, mergePage } from '../lib/notifications'

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
    /** Polls counts every 15s while the tab is visible; returns a stop function. */
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
            try {
                set({ counts: await notificationCounts() })
            } catch {
                // Keep the last known badge; never show a false zero.
            }
            if (get().open) await get().refresh()
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
        }),
}))
