import { describe, expect, it } from 'vitest'
import { backoffDelay, stateAfterFailures } from './socket'

describe('socket backoff', () => {
    it('doubles from 1 s up to 30 s', () => {
        expect([1, 2, 3, 4, 5, 6, 7].map(backoffDelay)).toEqual([1000, 2000, 4000, 8000, 16000, 30000, 30000])
    })

    it('goes offline after three failed attempts', () => {
        expect(stateAfterFailures(1)).toBe('reconnecting')
        expect(stateAfterFailures(2)).toBe('reconnecting')
        expect(stateAfterFailures(3)).toBe('offline')
    })
})
