import { describe, expect, it } from 'vitest'
import type { Activity } from '../api/endpoints/overview'
import { tzParam } from '../api/endpoints/overview'
import { greetingMeta, gridMeta, gridRows, hourHints, summaryLine } from './overview'

const hours = Array.from({ length: 24 }, (_, i) => new Date(Date.UTC(2026, 9, 22, i + 1)).toISOString())
const turns = (n: number) => Array.from({ length: 24 }, (_, i) => (i === 23 ? n : 0))

describe('tzParam', () => {
    it('writes the offset the API reads, east positive', () => {
        // Date#getTimezoneOffset is minutes WEST of UTC, so UTC+7 is -420.
        expect(tzParam({ getTimezoneOffset: () => -420 } as Date)).toBe('+07:00')
        expect(tzParam({ getTimezoneOffset: () => 330 } as Date)).toBe('-05:30')
        expect(tzParam({ getTimezoneOffset: () => 0 } as Date)).toBe('+00:00')
    })
})

describe('greetingMeta', () => {
    it('lists what is worth saying', () => {
        expect(greetingMeta({ agents: { total: 48, live: 2, failed: 1 }, cost_today_usd: 4.12 })).toBe('48 agents at court · 2 live · 1 failed · ~$4.12 today')
        expect(greetingMeta({ agents: { total: 1, live: 0, failed: 0 }, cost_today_usd: 0 })).toBe('1 agent at court')
    })
})

describe('hourHints', () => {
    it('names an hour and its turns', () => {
        const hints = hourHints(hours, turns(3))
        expect(hints).toHaveLength(24)
        expect(hints[23]).toMatch(/^\d\d:00 · 3 turns$/)
        expect(hints[0]).toMatch(/ · no turns$/)
        expect(hourHints(hours, turns(1))[23]).toMatch(/ · 1 turn$/)
    })
})

describe('the grid', () => {
    const base: Activity = {
        hours,
        rows: [{ agent_id: 'a', label: 'Archivist', turns: turns(2), live: true }],
        all: null,
        agent_total: 1,
        today_by_kind: { chat: 94, scheduled: 5, dream_stages: 2, background: 0 },
    }

    it('puts the All row first and marks it as the total', () => {
        const rows = gridRows({ ...base, agent_total: 9, all: { agent_id: null, label: 'All 9', turns: turns(7), live: false } })
        expect(rows.map((r) => [r.label, !!r.total])).toEqual([['All 9', true], ['Archivist', false]])
        expect(rows[0].hints).toHaveLength(24)
    })

    it('says the day in words for one agent', () => {
        expect(summaryLine(base)).toBe('Archivist is running now. 94 chat turns, 5 scheduled tasks and 2 dream stages today.')
        expect(summaryLine({ ...base, agent_total: 2 })).toBeUndefined()
    })

    it('titles the grid by how many rows it shows', () => {
        expect(gridMeta(base)).toBe('2 turns in 24 hours')
        expect(gridMeta({ ...base, agent_total: 4 })).toBe('4 agents · dot size = turns per hour')
        expect(gridMeta({ ...base, agent_total: 0, rows: [] })).toBeUndefined()
    })
})

describe('feedTime', () => {
    const now = new Date(2026, 9, 23, 22, 0)
    it('reads as a clock today, then in days', async () => {
        const { feedTime } = await import('./overview')
        expect(feedTime(new Date(2026, 9, 23, 7, 5).toISOString(), now)).toBe('07:05')
        expect(feedTime(new Date(2026, 9, 22, 23, 0).toISOString(), now)).toBe('yesterday')
        expect(feedTime(new Date(2026, 9, 12, 9, 0).toISOString(), now)).toBe('Oct 12')
    })
})
