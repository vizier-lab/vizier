import { describe, expect, it } from 'vitest'
import { abbreviate, cost, count, duration, relativeTime } from './format'

describe('format', () => {
    it('separates thousands and abbreviates for charts', () => {
        expect(count(61880)).toBe('61,880')
        expect(abbreviate(61880)).toBe('61.9k')
        expect(abbreviate(2_000_000)).toBe('2M')
        expect(abbreviate(950)).toBe('950')
    })

    it('formats cost', () => {
        expect(cost(0.03)).toBe('~$0.03')
        expect(cost(0.001)).toBe('<$0.01')
        expect(cost(0)).toBe('$0')
    })

    it('formats durations', () => {
        expect(duration(18_400)).toBe('18.4s')
        expect(duration(134_000)).toBe('2m 14s')
        expect(duration(3_900_000)).toBe('1h 05m')
    })

    it('formats relative time', () => {
        const now = Date.parse('2026-10-23T12:00:00Z')
        expect(relativeTime(now - 10_000, now)).toBe('just now')
        expect(relativeTime(now - 5 * 60_000, now)).toBe('5m ago')
        expect(relativeTime(now - 3 * 3_600_000, now)).toBe('3h ago')
        expect(relativeTime(now - 2 * 86_400_000, now)).toBe('2d ago')
    })
})
