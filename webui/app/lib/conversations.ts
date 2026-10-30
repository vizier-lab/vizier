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

export type SidebarGroup = 'Today' | 'Yesterday' | 'This week' | 'Earlier'

const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()

/** The sidebar's sections: the picker's two days, then the rest of the last week, then everything older. */
export function sidebarSections(items: ConversationItem[], now: Date = new Date()) {
    const groups: Record<SidebarGroup, ConversationItem[]> = { Today: [], Yesterday: [], 'This week': [], Earlier: [] }
    for (const item of items) {
        const days = Math.round((startOfDay(now) - startOfDay(new Date(item.last_activity))) / 86_400_000)
        groups[days <= 0 ? 'Today' : days === 1 ? 'Yesterday' : days < 7 ? 'This week' : 'Earlier'].push(item)
    }
    return (['Today', 'Yesterday', 'This week', 'Earlier'] as const).map((label) => ({ label, items: groups[label] })).filter((g) => g.items.length)
}

/** The sidebar's short time: "now", "12m", "2h" today, the clock for yesterday, the weekday this week, a date after. */
export function sidebarTime(at: string, now: Date = new Date()): string {
    const d = new Date(at)
    const mins = Math.floor((now.getTime() - d.getTime()) / 60_000)
    const days = Math.round((startOfDay(now) - startOfDay(d)) / 86_400_000)
    if (days <= 0) return mins < 1 ? 'now' : mins < 60 ? `${mins}m` : `${Math.floor(mins / 60)}h`
    if (days === 1) return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
    if (days < 7) return d.toLocaleDateString('en-US', { weekday: 'short' })
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

export const UNTITLED = 'Untitled conversation'

/** A conversation's name, or a generic one while it has none (the server says "Untitled", or leaves the topic's id). */
export function conversationTitle(title: string | undefined | null, topic?: string): string {
    const t = title?.trim()
    return !t || t === 'Untitled' || t === topic ? UNTITLED : t
}
