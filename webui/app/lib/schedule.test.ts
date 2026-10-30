import { describe, expect, it } from 'vitest'
import type { Task, TaskRun } from '../api/endpoints/tasks'
import {
    addDays,
    countTasks,
    dayChips,
    dayKey,
    dayView,
    filterTasks,
    momentState,
    monthWeeks,
    parseDayKey,
    scheduledAt,
} from './schedule'

const task = (over: Partial<Task> = {}): Task => ({
    slug: 't',
    title: 'T',
    instruction: 'do it',
    requester: { user: 'me' },
    is_active: true,
    schedule: { CronTask: '0 12 * * *' },
    timestamp: '2026-01-01T00:00:00Z',
    last_run: null,
    ...over,
})

const run = (over: Partial<TaskRun> = {}): TaskRun => ({ run_id: 'r', id: 1, ran_at: '2026-10-30T12:00:00Z', state: 'answered', ...over })

const utc = (s: string) => new Date(`${s}Z`)

describe('days', () => {
    it('round-trips a day key and refuses a date that is not real', () => {
        expect(dayKey(parseDayKey('2026-10-30')!)).toBe('2026-10-30')
        expect(parseDayKey('2026-02-30')).toBeNull()
        expect(parseDayKey('soon')).toBeNull()
    })

    it('lays a month out in Monday-first weeks that include the neighbouring days', () => {
        const weeks = monthWeeks(new Date(2026, 9, 15))
        expect(weeks).toHaveLength(5)
        expect(dayKey(weeks[0][0])).toBe('2026-09-28')
        expect(dayKey(weeks[4][6])).toBe('2026-11-01')
        expect(weeks.every((w) => w.length === 7)).toBe(true)
    })

    it('adds days by the calendar', () => {
        expect(dayKey(addDays(new Date(2026, 9, 31), 1))).toBe('2026-11-01')
    })
})

describe('scheduledAt', () => {
    it('lists a cron task in UTC', () => {
        const { at } = scheduledAt(task(), utc('2026-10-30T00:00:00'), utc('2026-11-01T00:00:00'))
        expect(at.map((d) => d.toISOString())).toEqual(['2026-10-30T12:00:00.000Z', '2026-10-31T12:00:00.000Z'])
    })

    it('includes the first instant of the range and leaves out the last', () => {
        const { at } = scheduledAt(task(), utc('2026-10-30T12:00:00'), utc('2026-10-31T12:00:00'))
        expect(at.map((d) => d.toISOString())).toEqual(['2026-10-30T12:00:00.000Z'])
    })

    it('puts a one-time task only inside its own range', () => {
        const once = task({ schedule: { OneTimeTask: '2026-10-31T08:30:00Z' } })
        expect(scheduledAt(once, utc('2026-10-31T00:00:00'), utc('2026-11-01T00:00:00')).at).toHaveLength(1)
        expect(scheduledAt(once, utc('2026-10-30T00:00:00'), utc('2026-10-31T00:00:00')).at).toHaveLength(0)
    })

    it('ignores an expression it cannot read', () => {
        expect(scheduledAt(task({ schedule: { CronTask: 'nonsense' } }), utc('2026-10-30T00:00:00'), utc('2026-11-01T00:00:00')).at).toEqual([])
    })

    it('stops at the cap and says so', () => {
        const every = task({ schedule: { CronTask: '* * * * *' } })
        const r = scheduledAt(every, utc('2026-10-30T00:00:00'), utc('2026-10-31T00:00:00'), 50)
        expect(r.at).toHaveLength(50)
        expect(r.capped).toBe(true)
    })
})

describe('momentState', () => {
    const now = utc('2026-10-30T12:04:00')

    it('takes the state of the run that happened there', () => {
        expect(momentState(task({ last_run: run({ state: 'no_response' }) }), utc('2026-10-30T12:00:00'), now)).toBe('no-response')
        expect(momentState(task({ last_run: run({ state: 'running' }) }), utc('2026-10-30T12:00:00'), now)).toBe('running')
    })

    it('calls an unrecorded past just past, and the future upcoming', () => {
        expect(momentState(task(), utc('2026-10-29T12:00:00'), now)).toBe('past')
        expect(momentState(task(), utc('2026-10-31T12:00:00'), now)).toBe('upcoming')
    })

    it('marks a paused task ahead as paused', () => {
        expect(momentState(task({ is_active: false }), utc('2026-10-31T12:00:00'), now)).toBe('paused')
    })
})

describe('filters', () => {
    const tasks = [
        task({ slug: 'a', title: 'Alpha', last_run: run({ state: 'no_response' }) }),
        task({ slug: 'b', title: 'Beta', is_active: false }),
        task({ slug: 'c', title: 'Gamma', last_run: run({ state: 'interrupted' }) }),
        task({ slug: 'd', title: 'Delta', last_run: run() }),
    ]

    it('applies the server\'s rules for each tab', () => {
        expect(filterTasks(tasks, 'active', '').map((t) => t.slug)).toEqual(['a', 'c', 'd'])
        expect(filterTasks(tasks, 'last_run_failed', '').map((t) => t.slug)).toEqual(['a', 'c'])
        expect(filterTasks(tasks, 'all', '')).toHaveLength(4)
    })

    it('searches the title and the slug, ignoring case', () => {
        expect(filterTasks(tasks, 'all', 'GAM').map((t) => t.slug)).toEqual(['c'])
        expect(filterTasks(tasks, 'all', 'b').map((t) => t.slug)).toContain('b')
    })

    it('counts every tab', () => {
        expect(countTasks(tasks)).toEqual({ active: 3, last_run_failed: 2, all: 4 })
    })
})

describe('dayChips', () => {
    const now = new Date(2026, 9, 30, 12, 4)
    const day = new Date(2026, 9, 31)

    it('draws a task that runs once as a single chip, and one that runs often as one repeat chip', () => {
        const chips = dayChips([task({ slug: 'once' }), task({ slug: 'often', title: 'Often', schedule: { CronTask: '0 */6 * * *' } })], day, now)
        expect(chips.map((c) => c.kind)).toEqual(['repeat', 'single'])
        const repeat = chips[0]
        expect(repeat.kind === 'repeat' && repeat.count).toBe(4)
    })

    it('leaves out a past moment nobody recorded', () => {
        expect(dayChips([task()], utc('2026-10-28T12:00:00'), now)).toEqual([])
    })

    it('keeps the run on record in the past', () => {
        const t = task({ last_run: run({ ran_at: '2026-10-28T12:00:00Z' }) })
        const [chip] = dayChips([t], utc('2026-10-28T12:00:00'), now)
        expect(chip.kind === 'single' && chip.state).toBe('answered')
    })

    it('puts repeats first, then the day in time order', () => {
        const a = task({ slug: 'a', title: 'A', schedule: { CronTask: '30 10 * * *' } })
        const b = task({ slug: 'b', title: 'B', schedule: { CronTask: '0 10 * * *' } })
        const chips = dayChips([a, b], day, now)
        expect(chips.map((c) => c.task.slug)).toEqual(['b', 'a'])
    })
})

describe('dayView', () => {
    const now = new Date(2026, 9, 30, 12, 4)
    const day = new Date(2026, 9, 31)

    it('keeps a task that runs a few times as blocks and in the repeat card', () => {
        const v = dayView([task({ schedule: { CronTask: '0 */6 * * *' } })], day, now)
        expect(v.blocks).toHaveLength(4)
        expect(v.repeats).toHaveLength(1)
        expect(v.repeats[0].dense).toBe(false)
    })

    it('keeps a busy task out of the timeline', () => {
        const v = dayView([task({ schedule: { CronTask: '*/30 * * * *' } })], day, now)
        expect(v.blocks).toHaveLength(0)
        expect(v.repeats[0].dense).toBe(true)
        expect(v.repeats[0].moments).toHaveLength(48)
    })

    it('orders the card by the soonest next run', () => {
        const slow = task({ slug: 'slow', title: 'Slow', schedule: { CronTask: '0 0,12 * * *' } })
        const quick = task({ slug: 'quick', title: 'Quick', schedule: { CronTask: '0 */4 * * *' } })
        const v = dayView([slow, quick], day, now)
        expect(v.repeats.map((r) => r.task.slug)).toContain('quick')
        expect(v.repeats).toHaveLength(2)
    })
})
