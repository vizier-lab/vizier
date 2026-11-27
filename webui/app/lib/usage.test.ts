import { expect, it } from 'vitest'
import { audioLength, dailyStacks, formatUnits, mediaCallLine, priceNote, rangeDates, shareOf, topSeries, unpricedMedia, versusPrevious } from './usage'

it('says a media amount in its own unit', () => {
    expect(formatUnits({ images: 233 })).toBe('233 images')
    expect(formatUnits({ images: 1 })).toBe('1 image')
    expect(formatUnits({ characters: 140_200, audio_seconds: 9000 })).toBe('140.2k chars · 2.5 h audio')
    expect(formatUnits({ audio_seconds: 38 })).toBe('38 s audio')
    expect(formatUnits({ tokens: 41_000_000 })).toBe('41M embedding tokens')
    expect(formatUnits(null)).toBeUndefined()
    expect(formatUnits({})).toBeUndefined()
    expect(audioLength(240)).toBe('4 min')
    expect(audioLength(3600)).toBe('1 h')
})

it('labels a price by where it came from', () => {
    expect(priceNote('local')).toBe('free · local')
    expect(priceNote('curated')).toMatch(/price table/)
    expect(priceNote('models_dev')).toBeUndefined()
    expect(unpricedMedia({ image: { images: 12 }, transcription: { audio_seconds: 60 } })).toBe('12 images, 1 min audio')
})

it('writes one line per media call on a step', () => {
    const call = { kind: 'image', provider: 'openai', model: 'gpt-image-1-mini', variant: '1024x1024', units: { images: 1 }, measured: true, local: false, cost_usd: 0.17, price_source: 'curated', at: '' } as const
    expect(mediaCallLine(call)).toBe('openai / gpt-image-1-mini · 1 image (1024x1024) · ~$0.17')
    expect(mediaCallLine({ ...call, local: true, cost_usd: 0, price_source: 'local' })).toMatch(/free · local$/)
    expect(mediaCallLine({ ...call, cost_usd: null, price_source: 'none' })).toMatch(/unpriced$/)
})

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
