import type {
    NotificationFilter,
    NotificationItem,
} from '../api/endpoints/notifications'

/** Does a row belong in the selected filter? */
export function belongs(filter: NotificationFilter, item: NotificationItem): boolean {
    if (filter === 'unread') return !item.read
    if (filter === 'read') return item.read
    return true
}

/**
 * Merge a freshly polled first page into what is already loaded: new rows go to
 * the top, rows already present are refreshed (a read flag changed in another
 * tab), and older pages stay. Ordering is by id, which is monotonic with creation
 * and breaks same-instant ties the way the server's cursor does.
 */
export function mergePage(
    existing: NotificationItem[],
    page: NotificationItem[],
    filter: NotificationFilter
): NotificationItem[] {
    const byId = new Map<number, NotificationItem>()
    for (const item of [...page, ...existing]) {
        if (!belongs(filter, item)) continue
        if (!byId.has(item.id)) byId.set(item.id, item)
    }
    return [...byId.values()].sort((a, b) => b.id - a.id)
}

/**
 * Apply this person's read choice to the loaded rows, dropping a row that no
 * longer belongs in the selected filter (read leaves Unread; unread leaves Read).
 */
export function applyRead(
    items: NotificationItem[],
    id: number,
    read: boolean,
    filter: NotificationFilter
): NotificationItem[] {
    return items.flatMap((item) => {
        if (item.id !== id) return [item]
        if (!belongs(filter, { ...item, read })) return []
        return [{ ...item, read }]
    })
}

/** Every loaded row becomes read; under Unread the list empties. */
export function applyAllRead(
    items: NotificationItem[],
    filter: NotificationFilter
): NotificationItem[] {
    if (filter === 'unread') return []
    return items.map((item) => ({ ...item, read: true }))
}

/** The entry's accessible name, so the badge is not the only carrier. */
export function entryLabel(unread: number, busy: boolean): string {
    const parts = ['Notifications']
    if (unread > 0) parts.push(`${unread} unread`)
    if (busy) parts.push('setting up')
    return parts.join(', ')
}

/**
 * Is this row the conversation the person is looking at right now? A WebUI reply for the
 * open conversation needs no toast and no badge bump: it is already on screen.
 */
export function isViewing(
    item: NotificationItem,
    path: string | null | undefined
): boolean {
    if (!path || item.category !== 'chat_reply') return false
    const strip = (value: string) => value.replace(/\/+$/, '')
    return strip(item.href) === strip(path)
}
