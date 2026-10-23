import type { ConversationItem } from '../api/endpoints/conversations'

export type DayGroup = 'Today' | 'Yesterday' | 'Older'

/** Which of the picker's sections a conversation belongs in. */
export function dayGroup(at: string, now: Date = new Date()): DayGroup {
    const day = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
    const days = Math.round((day(now) - day(new Date(at))) / 86_400_000)
    return days <= 0 ? 'Today' : days === 1 ? 'Yesterday' : 'Older'
}

/** The sections in display order, leaving out the empty ones. */
export function sections(items: ConversationItem[], now: Date = new Date()) {
    const groups: Record<DayGroup, ConversationItem[]> = { Today: [], Yesterday: [], Older: [] }
    for (const item of items) groups[dayGroup(item.last_activity, now)].push(item)
    return (['Today', 'Yesterday', 'Older'] as const).map((label) => ({ label, items: groups[label] })).filter((g) => g.items.length)
}

/** "14:02" for today and yesterday's rows, a short date for older ones. */
export function rowTime(at: string, now: Date = new Date()): string {
    const d = new Date(at)
    if (dayGroup(at, now) === 'Older') return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
    return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}

/** Where a conversation opens: a WebUI topic in the chat, a Discord or Telegram one read-only. */
export function conversationHref(agentId: string, c: ConversationItem): string {
    return c.kind === 'webui'
        ? `/${agentId}/chat/${encodeURIComponent(c.latest_topic)}`
        : `/${agentId}/chat/read?channel=${encodeURIComponent(c.channel)}`
}
