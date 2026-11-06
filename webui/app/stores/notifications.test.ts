import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { NotificationItem, NotificationPage } from '../api/endpoints/notifications'

vi.mock('../api/endpoints/notifications', async (importOriginal) => {
    const actual =
        await importOriginal<typeof import('../api/endpoints/notifications')>()
    return {
        ...actual,
        listNotifications: vi.fn(),
        notificationCounts: vi.fn(),
        setNotificationRead: vi.fn(),
        markAllNotificationsRead: vi.fn(),
    }
})

import * as api from '../api/endpoints/notifications'
import { useNotifications } from './notifications'

const list = vi.mocked(api.listNotifications)
const readOne = vi.mocked(api.setNotificationRead)
const readAll = vi.mocked(api.markAllNotificationsRead)

const counts = { all: 3, unread: 2, read: 1 }

function item(id: number, read = false): NotificationItem {
    return {
        id,
        at: '2026-10-25T09:14:02Z',
        agent_id: 'scribe',
        category: 'task_run',
        tone: 'default',
        text: `Notification ${id}`,
        href: '/scribe/tasks',
        read,
    }
}

function page(items: NotificationItem[], next: string | null = null): NotificationPage {
    return { items, counts, next }
}

beforeEach(() => {
    vi.clearAllMocks()
    useNotifications.getState().reset()
})

describe('the notifications store', () => {
    it('shows what is loaded as soon as the store is usable', () => {
        expect(useNotifications.getState().items).toEqual([])
        expect(useNotifications.getState().counts).toEqual({ all: 0, unread: 0, read: 0 })
    })

    it('loads the first page and keeps the cursor', async () => {
        list.mockResolvedValue(page([item(2), item(1)], 'cursor'))
        useNotifications.setState({ loaded: true })
        useNotifications.getState().openPanel()
        await vi.waitFor(() =>
            expect(useNotifications.getState().items).toHaveLength(2)
        )
        expect(useNotifications.getState().open).toBe(true)
        expect(useNotifications.getState().next).toBe('cursor')
        expect(useNotifications.getState().counts).toEqual(counts)
    })

    it('appends older pages without duplicating a row', async () => {
        list.mockResolvedValueOnce(page([item(3), item(2)], 'a'))
        await useNotifications.getState().load()
        list.mockResolvedValueOnce(page([item(2), item(1)], null))
        await useNotifications.getState().loadMore()
        expect(useNotifications.getState().items.map((i) => i.id)).toEqual([3, 2, 1])
        expect(useNotifications.getState().next).toBeNull()
    })

    it('marks one read optimistically and drops it from Unread', async () => {
        list.mockResolvedValue(page([item(2), item(1)]))
        useNotifications.setState({ filter: 'unread' })
        await useNotifications.getState().load()
        readOne.mockResolvedValue({ all: 3, unread: 1, read: 2 })

        await useNotifications.getState().markRead(2, true)
        expect(readOne).toHaveBeenCalledWith(2, true)
        expect(useNotifications.getState().items.map((i) => i.id)).toEqual([1])
        expect(useNotifications.getState().counts).toEqual({ all: 3, unread: 1, read: 2 })
    })

    it('restores the row and counts when a read request fails', async () => {
        list.mockResolvedValue(page([item(2), item(1)]))
        useNotifications.setState({ filter: 'unread' })
        await useNotifications.getState().load()
        readOne.mockRejectedValue(new Error('offline'))

        await useNotifications.getState().markRead(2, true)
        expect(useNotifications.getState().items.map((i) => i.id)).toEqual([2, 1])
        expect(useNotifications.getState().counts.unread).toBe(2)
    })

    it('marks every loaded row read and empties Unread', async () => {
        list.mockResolvedValue(page([item(2), item(1)]))
        useNotifications.setState({ filter: 'unread' })
        await useNotifications.getState().load()
        readAll.mockResolvedValue({ all: 3, unread: 0, read: 3 })

        await useNotifications.getState().markAll()
        expect(readAll).toHaveBeenCalled()
        expect(useNotifications.getState().items).toEqual([])
        expect(useNotifications.getState().counts.unread).toBe(0)
    })

    it('keeps the last counts when a load fails rather than showing zero', async () => {
        list.mockRejectedValue(new Error('down'))
        useNotifications.setState({ counts: { all: 5, unread: 4, read: 1 } })
        await useNotifications.getState().load()
        expect(useNotifications.getState().error).toBeTruthy()
        expect(useNotifications.getState().counts).toEqual({ all: 5, unread: 4, read: 1 })
    })

    it('turns pushed rows above the high-water mark into arrivals, never the first snapshot', () => {
        const { receive } = useNotifications.getState()
        receive({ type: 'hello', counts, latest_id: 2, setups: [] })
        expect(useNotifications.getState().primed).toBe(true)
        expect(useNotifications.getState().lastSeenId).toBe(2)
        expect(useNotifications.getState().arrived).toEqual([])

        receive({ type: 'notification', item: item(3), counts })
        expect(useNotifications.getState().arrived.map((i) => i.id)).toEqual([3])
        expect(useNotifications.getState().lastSeenId).toBe(3)
        expect(useNotifications.getState().takeArrived()).toHaveLength(1)
        expect(useNotifications.getState().takeArrived()).toEqual([])
        // The same row again is not new.
        receive({ type: 'notification', item: item(3), counts })
        expect(useNotifications.getState().arrived).toEqual([])
    })

    it('reads a reply to the conversation on screen with no toast and no badge bump', () => {
        const { receive } = useNotifications.getState()
        receive({ type: 'hello', counts: { all: 1, unread: 1, read: 0 }, latest_id: 1, setups: [] })
        useNotifications.setState({ viewing: '/scribe/chat/t1' })
        readOne.mockResolvedValue({ all: 2, unread: 1, read: 1 })
        const reply = { ...item(2), category: 'chat_reply' as const, href: '/scribe/chat/t1' }
        receive({ type: 'notification', item: reply, counts: { all: 2, unread: 2, read: 0 } })
        expect(useNotifications.getState().arrived).toEqual([])
        expect(readOne).toHaveBeenCalledWith(2, true)
        expect(useNotifications.getState().counts).toEqual({ all: 2, unread: 1, read: 1 })
    })

    it('toasts a reply for a different conversation', () => {
        const { receive } = useNotifications.getState()
        receive({ type: 'hello', counts, latest_id: 1, setups: [] })
        useNotifications.setState({ viewing: '/scribe/chat/other' })
        const reply = { ...item(2), category: 'chat_reply' as const, href: '/scribe/chat/t1' }
        receive({ type: 'notification', item: reply, counts })
        expect(useNotifications.getState().arrived.map((i) => i.id)).toEqual([2])
        expect(readOne).not.toHaveBeenCalled()
    })

    it('does not toast a row that arrives while the panel is open, and puts it at the top', () => {
        const { receive } = useNotifications.getState()
        receive({ type: 'hello', counts, latest_id: 1, setups: [] })
        useNotifications.setState({ open: true, loaded: true, filter: 'all', items: [item(1)] })
        receive({ type: 'notification', item: item(2), counts })
        expect(useNotifications.getState().arrived).toEqual([])
        expect(useNotifications.getState().items.map((i) => i.id)).toEqual([2, 1])
    })

    it('applies a read frame from another client', () => {
        useNotifications.setState({ filter: 'unread', items: [item(2), item(1)], loaded: true })
        useNotifications.getState().receive({
            type: 'read',
            id: 2,
            read: true,
            counts: { all: 3, unread: 1, read: 2 },
        })
        expect(useNotifications.getState().items.map((i) => i.id)).toEqual([1])
        expect(useNotifications.getState().counts.unread).toBe(1)
    })

    it('fetches the newest rows when a reconnect shows it missed some', async () => {
        const { receive } = useNotifications.getState()
        receive({ type: 'hello', counts, latest_id: 2, setups: [] })
        list.mockResolvedValue(page([item(4), item(3), item(2)]))
        receive({ type: 'hello', counts: { all: 5, unread: 4, read: 1 }, latest_id: 4, setups: [] })
        await vi.waitFor(() => expect(useNotifications.getState().lastSeenId).toBe(4))
        expect(useNotifications.getState().arrived.map((i) => i.id)).toEqual([4, 3])
    })

    it('a resync replaces the loaded first page so deleted rows go away', async () => {
        useNotifications.setState({ filter: 'all', loaded: true, primed: true, lastSeenId: 3, items: [item(3), item(2)] })
        list.mockResolvedValue(page([item(2)]))
        useNotifications.getState().receive({ type: 'resync' })
        await vi.waitFor(() => expect(useNotifications.getState().items.map((i) => i.id)).toEqual([2]))
    })

    it('opens on Unread, and a sign-out returns it there', () => {
        useNotifications.getState().setFilter('all')
        useNotifications.getState().reset()
        expect(useNotifications.getState().filter).toBe('unread')
    })

    it('merges a poll at the top and keeps older pages', async () => {
        list.mockResolvedValueOnce(page([item(3), item(2)], 'older'))
        useNotifications.setState({ loaded: true, filter: 'all' })
        await useNotifications.getState().load()
        list.mockResolvedValueOnce(page([item(4, true), item(3)], null))
        await useNotifications.getState().refresh()
        expect(useNotifications.getState().items.map((i) => i.id)).toEqual([4, 3, 2])
        expect(useNotifications.getState().next).toBe('older')
    })

    it('the bell toggles the popup, and a second press closes it', () => {
        list.mockResolvedValue(page([]))
        useNotifications.setState({ loaded: true })
        useNotifications.getState().togglePanel()
        expect(useNotifications.getState().open).toBe(true)
        useNotifications.getState().togglePanel()
        expect(useNotifications.getState().open).toBe(false)
    })

    it('opens with no bell mounted, and keeps the bell it was given as its anchor', () => {
        list.mockResolvedValue(page([]))
        useNotifications.setState({ loaded: true })
        expect(useNotifications.getState().anchor).toBeNull()
        useNotifications.getState().openPanel()
        expect(useNotifications.getState().open).toBe(true)
        const bell = { isConnected: true } as unknown as HTMLElement
        useNotifications.getState().setAnchor(bell)
        expect(useNotifications.getState().anchor).toBe(bell)
    })

    it('a filter change asks the server for that filter only', async () => {
        list.mockResolvedValue(page([item(1)]))
        useNotifications.getState().setFilter('all')
        await vi.waitFor(() => expect(list).toHaveBeenCalledWith('all', undefined, 20))
        expect(list).not.toHaveBeenCalledWith('read', expect.anything(), expect.anything())
    })
})
