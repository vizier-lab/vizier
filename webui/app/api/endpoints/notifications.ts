import { api } from '../client'

export type NotificationFilter = 'all' | 'unread' | 'read'

export type NotificationCategory =
    | 'agent_start_failure'
    | 'agent_setup_ready'
    | 'task_run'
    | 'dream_cycle'
    | 'background_work'
    | 'integration'
    | 'chat_reply'
    | 'workspace_message'

export type NotificationTone = 'default' | 'failed' | 'success'

export type NotificationItem = {
    id: number
    at: string
    agent_id: string
    category: NotificationCategory
    tone: NotificationTone
    /** Plain text: render it as text, never as HTML or Markdown. */
    text: string
    href: string
    read: boolean
    /** The conversation's current title, or the task's. Absent when there is none or it isn't yours to see. */
    title?: string
    /** A short plain-text excerpt of what the source said. Render it as text. */
    excerpt?: string
}

export type NotificationCounts = { all: number; unread: number; read: number }

export type NotificationPage = {
    items: NotificationItem[]
    counts: NotificationCounts
    next?: string | null
}

export const listNotifications = (
    filter: NotificationFilter = 'all',
    before?: string | null,
    limit = 20
) => {
    const qs = new URLSearchParams({ filter, limit: String(limit) })
    if (before) qs.set('before', before)
    return api.get<NotificationPage>(`/notifications?${qs.toString()}`)
}

export const notificationCounts = () =>
    api.get<NotificationCounts>('/notifications/count')

export const setNotificationRead = (id: number, read: boolean) =>
    api.put<NotificationCounts>(`/notifications/${id}/read`, { read })

export const markAllNotificationsRead = () =>
    api.post<NotificationCounts>('/notifications/read-all')

/** Replies not yet read in one of this person's conversations. */
export type ChatUnread = { agent_id: string; href: string; unread: number }

export const unreadChats = () => api.get<ChatUnread[]>('/notifications/chats')

export const markChatRead = (href: string) =>
    api.post<NotificationCounts>('/notifications/chats/read', { href })
