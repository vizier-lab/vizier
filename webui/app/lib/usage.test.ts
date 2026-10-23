import { expect, it } from 'vitest'
import { dailyStacks, rangeDates, shareOf, topSeries, versusPrevious } from './usage'

it('takes the last 7 or 30 days, or what was picked', () => {
    const now = new Date('2026-10-23T12:00:00Z')
    expect(rangeDates('7', now).start.toISOString()).toBe('2026-10-16T12:00:00.000Z')
    expect(rangeDates('30', now).start.toISOString()).toBe('2026-09-23T12:00:00.000Z')
    const from = new Date('2026-01-01'), to = new Date('2026-02-01')
    expect(rangeDates('custom', now, { from, to })).toEqual({ start: from, end: to })
})

it('keeps the top five and folds the rest', () => {
    const items = Array.from({ length: 8 }, (_, i) => ({ key: `k${i}`, label: `k${i}`, v: i }))
    const { top, other } = topSeries(items, (i) => i.v)
    expect(top.map((i) => i.key)).toEqual(['k7', 'k6', 'k5', 'k4', 'k3'])
    expect(other).toEqual({ count: 3, value: 0 + 1 + 2 })
    expect(topSeries(items.slice(0, 3), (i) => i.v).other).toBeNull()
})

it('stacks a day by activity, folding the others', () => {
    const daily = [{ date: '2026-10-20', by_activity: { chat: { calls: 1, tokens: 10, cost_usd: 0.5 }, dreams: { calls: 1, tokens: 5, cost_usd: 0.25 }, vision: { calls: 1, tokens: 1, cost_usd: null } } }]
    expect(dailyStacks(daily as never, 'cost', ['chat'])[0].values).toEqual({ chat: 0.5, other: 0.25 })
    expect(dailyStacks(daily as never, 'tokens', ['chat', 'dreams'])[0].values).toEqual({ chat: 10, dreams: 5, other: 1 })
})

it('shares and comparisons', () => {
    const rows = [{ key: 'a', label: 'a', tokens: 30, calls: 1, cost_usd: 3 }, { key: 'b', label: 'b', tokens: 10, calls: 1, cost_usd: 1 }]
    expect(shareOf(rows, rows[0], 'cost')).toBeCloseTo(0.75)
    expect(shareOf(rows, rows[1], 'tokens')).toBeCloseTo(0.25)
    expect(versusPrevious(112, 100, 'previous 30 days')).toBe('+12% vs previous 30 days')
    expect(versusPrevious(50, 100, 'x')).toBe('-50% vs x')
    expect(versusPrevious(5, 0, 'x')).toBeUndefined()
})
