import { describe, expect, it } from 'vitest'
import { isUnreachable } from './connectivity'

describe('when the app cannot reach the engine', () => {
    it('is so when the browser says it is offline', () => {
        expect(isUnreachable(true, {})).toBe(true)
        expect(isUnreachable(false, {})).toBe(false)
    })

    it('is so when a topic has lost its connection', () => {
        expect(isUnreachable(false, { 'oz/topic-1': 'offline' })).toBe(true)
    })

    it('is not so while a topic is only reconnecting', () => {
        expect(isUnreachable(false, { 'oz/topic-1': 'reconnecting' })).toBe(false)
        expect(isUnreachable(false, { 'oz/topic-1': 'connecting' })).toBe(false)
        expect(isUnreachable(false, { 'oz/topic-1': 'open' })).toBe(false)
    })
})
