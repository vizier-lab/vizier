import { afterEach, describe, expect, it, vi } from 'vitest'
import { INTERACTION_WINDOW_MS, SendGate, StateCoalescer, parseFrameMessage, themeTokens } from './host'

describe('parseFrameMessage', () => {
    it('accepts the runtime messages and nothing else', () => {
        expect(parseFrameMessage({ vz: 1, type: 'heartbeat' })).toEqual({ type: 'heartbeat' })
        expect(parseFrameMessage({ vz: 1, type: 'height', px: 120.2 })).toEqual({ type: 'height', px: 121 })
        expect(parseFrameMessage({ vz: 1, type: 'send', id: 3, text: '  e2e4 ', data: { from: 'e2' } })).toEqual({
            type: 'send',
            id: 3,
            text: 'e2e4',
            data: { from: 'e2' },
        })
        expect(parseFrameMessage({ vz: 2, type: 'heartbeat' })).toBeNull()
        expect(parseFrameMessage({ type: 'vizier:send', text: 'old protocol' })).toBeNull()
        expect(parseFrameMessage({ vz: 1, type: 'height', px: 'tall' })).toBeNull()
        expect(parseFrameMessage({ vz: 1, type: 'navigate' })).toBeNull()
    })

    it('clips a problem and refuses unknown kinds', () => {
        const long = 'x'.repeat(2000)
        const parsed = parseFrameMessage({ vz: 1, type: 'problem', problem: { kind: 'error', message: long, line: 3 } })
        expect(parsed).toEqual({ type: 'problem', problem: { kind: 'error', message: 'x'.repeat(500), source: 'page', line: 3 } })
        expect(parseFrameMessage({ vz: 1, type: 'problem', problem: { kind: 'evil', message: 'x' } })).toBeNull()
    })
})

describe('SendGate', () => {
    it('needs a recent interaction and allows one send a second', () => {
        const gate = new SendGate()
        expect(gate.check(1_000, false)).toBe('no_interaction')
        gate.interacted(1_000)
        expect(gate.check(1_100, false)).toBeNull()
        expect(gate.check(1_500, false)).toBe('rate_limited')
        expect(gate.check(2_200, false)).toBeNull()
        expect(gate.check(1_000 + INTERACTION_WINDOW_MS + 1, false)).toBe('no_interaction')
        expect(gate.check(1_000 + INTERACTION_WINDOW_MS + 1, true)).toBeNull()
    })
})

describe('StateCoalescer', () => {
    afterEach(() => vi.useRealTimers())

    it('saves the last value of each window', () => {
        vi.useFakeTimers()
        const saved: unknown[] = []
        const c = new StateCoalescer((v) => saved.push(v), 500)
        c.push(1)
        c.push(2)
        c.push(3)
        vi.advanceTimersByTime(499)
        expect(saved).toEqual([])
        vi.advanceTimersByTime(1)
        expect(saved).toEqual([3])
        c.push(4)
        c.dispose()
        vi.advanceTimersByTime(1000)
        expect(saved).toEqual([3])
    })
})

describe('themeTokens', () => {
    it('reads the role tokens that have a value', () => {
        const values: Record<string, string> = { '--vz-ground': ' #fff ', '--vz-text': '#111114' }
        const tokens = themeTokens({ getPropertyValue: (name: string) => values[name] ?? '' })
        expect(tokens).toEqual({ '--vz-ground': '#fff', '--vz-text': '#111114' })
    })
})
