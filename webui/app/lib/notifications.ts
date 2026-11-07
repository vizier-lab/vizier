import type {
    ChatUnread,
    NotificationFilter,
    NotificationItem,
} from '../api/endpoints/notifications'

/** One spelling of an in-app path: no query, no trailing slash, nothing percent-encoded. */
function pathKey(path: string): string {
    const bare = path.split('?')[0].replace(/\/+$/, '')
    try {
        return decodeURIComponent(bare)
    } catch {
        return bare
    }
}

/** Do these two in-app paths lead to the same place? */
export function samePath(a: string, b: string): boolean {
    return pathKey(a) === pathKey(b)
}

/** Unread replies per agent, for the rail. */
export function unreadByAgent(chats: ChatUnread[]): Record<string, number> {
    const counts: Record<string, number> = {}
    for (const chat of chats) counts[chat.agent_id] = (counts[chat.agent_id] ?? 0) + chat.unread
    return counts
}

/** Unread replies in the conversation at `href`, for its row. */
export function unreadAt(chats: ChatUnread[], href: string): number {
    return chats.find((chat) => samePath(chat.href, href))?.unread ?? 0
}

/** What the popup offers; the REST `read` filter stays for other clients. */
export type ViewFilter = Exclude<NotificationFilter, 'read'>

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
    // A reply in the open conversation, or a message in the Workspace topic on screen.
    if (!path || (item.category !== 'chat_reply' && item.category !== 'workspace_message')) return false
    return samePath(item.href, path)
}
