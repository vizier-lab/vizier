import { describe, expect, it } from 'vitest'
import { IDLE_STOP_MS, MIN_INTERVAL_MS, shouldTick } from './control'

describe('Effect ticking', () => {
    const now = 1_000_000

    it('ticks while running, visible, on screen and the person has been about', () => {
        expect(shouldTick(now, now - 1000, true, true, true)).toBe(true)
    })

    it.each([
        ['paused', [now - 1000, true, true, false]],
        ['tab hidden', [now - 1000, false, true, true]],
        ['off screen', [now - 1000, true, false, true]],
        ['idle for 10 minutes', [now - IDLE_STOP_MS, true, true, true]],
    ] as const)('stops when %s', (_, [last, visible, onScreen, running]) => {
        expect(shouldTick(now, last, visible, onScreen, running)).toBe(false)
    })

    it('never ticks faster than every 50 ms', () => {
        expect(MIN_INTERVAL_MS).toBe(50)
    })
})
