import { describe, expect, it } from 'vitest'
import type { ConversationItem } from '../api/endpoints/conversations'
import { conversationHref, dayGroup, rowTime, sections } from './conversations'

const now = new Date(2026, 9, 23, 12, 0)
const item = (channel: string, kind: ConversationItem['kind'], at: Date, topic = ''): ConversationItem => ({
    channel, kind, title: channel, place: '', topics: 1, last_activity: at.toISOString(), latest_topic: topic,
})

describe('sections', () => {
    it('groups by day, newest first within the order given, and drops empty ones', () => {
        const groups = sections(
            [
                item('a', 'webui', new Date(2026, 9, 23, 9, 0)),
                item('b', 'discord', new Date(2026, 9, 22, 20, 0)),
                item('c', 'telegram', new Date(2026, 9, 1, 8, 0)),
                item('d', 'webui', new Date(2026, 9, 23, 8, 0)),
            ],
            now
        )
        expect(groups.map((g) => [g.label, g.items.map((i) => i.channel)])).toEqual([
            ['Today', ['a', 'd']],
            ['Yesterday', ['b']],
            ['Older', ['c']],
        ])
        expect(sections([item('a', 'webui', new Date(2026, 9, 23, 9, 0))], now).map((g) => g.label)).toEqual(['Today'])
    })
})

describe('times', () => {
    it('is a clock for recent rows and a date for old ones', () => {
        expect(rowTime(new Date(2026, 9, 23, 14, 2).toISOString(), now)).toBe('14:02')
        expect(rowTime(new Date(2026, 9, 5, 14, 2).toISOString(), now)).toBe('Oct 5')
        expect(dayGroup(new Date(2026, 9, 22, 1, 0).toISOString(), now)).toBe('Yesterday')
    })
})

describe('conversationHref', () => {
    it('opens a WebUI topic in the chat and anything else read-only', () => {
        expect(conversationHref('scribe', item('http__ana__vizier-webui', 'webui', now, 't 1'))).toBe('/scribe/chat/t%201')
        expect(conversationHref('scribe', item('discord__42', 'discord', now))).toBe('/scribe/chat/read?channel=discord__42')
    })
})
