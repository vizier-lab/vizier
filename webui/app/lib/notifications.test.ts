import { describe, expect, it } from 'vitest'
import {
    applyAllRead,
    applyRead,
    belongs,
    entryLabel,
    isViewing,
    mergePage,
} from './notifications'
import type { NotificationItem } from '../api/endpoints/notifications'

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

describe('notifications helpers', () => {
    it('merges a polled page at the top without duplicating older rows', () => {
        const existing = [item(5), item(4), item(3)]
        const page = [item(7), item(6), item(5, true)]
        const merged = mergePage(existing, page, 'all')
        expect(merged.map((i) => i.id)).toEqual([7, 6, 5, 4, 3])
        expect(merged.find((i) => i.id === 5)?.read).toBe(true)
    })

    it('keeps only rows the selected filter admits', () => {
        const existing = [item(3), item(2)]
        expect(mergePage(existing, [item(5, true), item(4)], 'unread').map((i) => i.id)).toEqual([4, 3, 2])
        expect(mergePage(existing, [item(5, true), item(4)], 'read').map((i) => i.id)).toEqual([5])
    })

    it('drops a row from Unread when it is read, and from Read when it is unread', () => {
        expect(applyRead([item(2), item(1)], 2, true, 'unread').map((i) => i.id)).toEqual([1])
        expect(applyRead([item(2, true), item(1)], 2, false, 'read').map((i) => i.id)).toEqual([1])
        expect(applyRead([item(2), item(1)], 2, true, 'all')[0]).toMatchObject({ id: 2, read: true })
    })

    it('marks every loaded row read and empties Unread', () => {
        expect(applyAllRead([item(2), item(1)], 'all').every((i) => i.read)).toBe(true)
        expect(applyAllRead([item(2), item(1)], 'unread')).toEqual([])
    })

    it('states the count and busy state in words', () => {
        expect(entryLabel(3, false)).toBe('Notifications, 3 unread')
        expect(entryLabel(0, true)).toBe('Notifications, setting up')
        expect(entryLabel(0, false)).toBe('Notifications')
    })

    it('knows the conversation being looked at', () => {
        const reply = { ...item(9), category: 'chat_reply' as const, href: '/scribe/chat/t1' }
        expect(isViewing(reply, '/scribe/chat/t1')).toBe(true)
        expect(isViewing(reply, '/scribe/chat/t1/')).toBe(true)
        expect(isViewing(reply, '/scribe/chat/t2')).toBe(false)
        expect(isViewing(reply, '/scribe/core')).toBe(false)
        expect(isViewing(reply, null)).toBe(false)
        // Other categories are never the conversation on screen.
        expect(isViewing(item(1), '/scribe/chat/t1')).toBe(false)
    })

    it('filters by read state', () => {
        expect(belongs('all', item(1, true))).toBe(true)
        expect(belongs('unread', item(1, false))).toBe(true)
        expect(belongs('unread', item(1, true))).toBe(false)
        expect(belongs('read', item(1, true))).toBe(true)
        expect(belongs('read', item(1, false))).toBe(false)
    })
})
