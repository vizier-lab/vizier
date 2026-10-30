import { describe, expect, it } from 'vitest'
import { MIN_GAP_MS, playNotify, shouldPlay } from './sound'

describe('shouldPlay', () => {
    it('plays the first sound', () => {
        expect(shouldPlay(1000, null)).toBe(true)
    })

    it('folds sounds closer than the gap into one', () => {
        expect(shouldPlay(1000 + MIN_GAP_MS - 1, 1000)).toBe(false)
        expect(shouldPlay(1000 + MIN_GAP_MS, 1000)).toBe(true)
    })
})

describe('playNotify', () => {
    it('does nothing where there is no audio', () => {
        expect(() => playNotify()).not.toThrow()
    })
})
