import { describe, expect, it } from 'vitest'
import { MINI_DOTS_PER_STATE, miniDots, type MiniDot } from './trail'

const dots = (n: number, dot: MiniDot = 'done') => Array.from({ length: n }, () => dot)

describe('the phone trail\'s dots', () => {
    it('passes a short trail through untouched', () => {
        expect(miniDots(['done', 'failed', 'done'])).toEqual({ dots: ['done', 'failed', 'done'], more: 0 })
    })

    it('keeps at most a few of each state and counts the rest', () => {
        const { dots: kept, more } = miniDots(dots(120))
        expect(kept).toHaveLength(MINI_DOTS_PER_STATE)
        expect(more).toBe(120 - MINI_DOTS_PER_STATE)
    })

    it('counts each state on its own, so a failure is never crowded out by successes', () => {
        const { dots: kept, more } = miniDots([...dots(50), 'failed'])
        expect(kept).toContain('failed')
        expect(kept.filter((d) => d === 'done')).toHaveLength(MINI_DOTS_PER_STATE)
        expect(more).toBe(50 - MINI_DOTS_PER_STATE)
    })

    it('keeps the newest calls, in the order they happened', () => {
        // Nine failures, the oldest far behind the cap: one of them is counted out, newest kept.
        const { dots: kept, more } = miniDots([
            'failed',
            ...dots(30),
            ...dots(MINI_DOTS_PER_STATE, 'failed'),
            'running',
        ])
        expect(kept.filter((d) => d === 'failed')).toHaveLength(MINI_DOTS_PER_STATE)
        expect(kept[kept.length - 1]).toBe('running')
        // The evicted one is the oldest failure, so no 'failed' sits at the very front.
        expect(kept[0]).toBe('done')
        expect(more).toBe(1 + 30 - MINI_DOTS_PER_STATE)
    })
})
