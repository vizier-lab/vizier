import { describe, expect, it } from 'vitest'
import {
    charsLeft,
    isFull,
    MAX_SIDE_CHAT_CHARS,
    initialSideChat,
    mapAskError,
    refusal,
    sideChatReducer,
    toTurns,
} from './sideChat'

const q = (n: number) => 'a'.repeat(n)

describe('sideChatReducer', () => {
    it('asks, answers and keeps the turn order', () => {
        let s = sideChatReducer(initialSideChat, { type: 'ask', question: ' first ' })
        expect(s.pending).toBe(0)
        expect(s.turns[0].question).toBe('first')
        s = sideChatReducer(s, { type: 'answered', index: 0, answer: 'one' })
        expect(s.pending).toBeNull()
        expect(s.turns).toEqual([{ question: 'first', answer: 'one', error: undefined }])
    })

    it('marks a failed ask and retries the same turn', () => {
        let s = sideChatReducer(initialSideChat, { type: 'ask', question: 'q' })
        s = sideChatReducer(s, { type: 'failed', index: 0, message: 'boom', retryable: true })
        expect(s.turns[0].error).toEqual({ message: 'boom', retryable: true })
        expect(s.pending).toBeNull()
        s = sideChatReducer(s, { type: 'retry', index: 0 })
        expect(s.turns[0].error).toBeUndefined()
        expect(s.pending).toBe(0)
    })

    it('ignores an answer for a turn that is not pending', () => {
        let s = sideChatReducer(initialSideChat, { type: 'ask', question: 'q' })
        s = sideChatReducer(s, { type: 'answered', index: 0, answer: 'a' })
        s = sideChatReducer(s, { type: 'ask', question: 'next' })
        s = sideChatReducer(s, { type: 'answered', index: 0, answer: 'late' })
        expect(s.pending).toBe(1)
        expect(s.turns[0].answer).toBe('a')
    })

    it('reset clears everything', () => {
        const s = sideChatReducer(initialSideChat, { type: 'ask', question: 'q' })
        expect(sideChatReducer(s, { type: 'reset' })).toEqual(initialSideChat)
    })
})

describe('character budget', () => {
    it('counts questions and answers, and sends only answered turns', () => {
        let s = sideChatReducer(initialSideChat, { type: 'ask', question: 'hi' })
        s = sideChatReducer(s, { type: 'answered', index: 0, answer: 'there' })
        s = sideChatReducer(s, { type: 'ask', question: 'pending' })
        expect(charsLeft(s.turns)).toBe(MAX_SIDE_CHAT_CHARS - 2 - 5 - 7)
        expect(toTurns(s.turns)).toEqual([{ question: 'hi', answer: 'there' }])
    })

    it('refuses an empty question, a long one, and one that does not fit', () => {
        expect(refusal([], '   ')).toMatch(/Type a question/)
        expect(refusal([], q(4_001))).toMatch(/4,000/)
        expect(refusal([], q(4_000))).toBeNull()
        const full = [{ question: q(4_000), answer: q(27_999) }, { question: q(4_000), answer: q(1) }]
        expect(refusal(full, q(1))).toMatch(/full/)
    })

    it('is full only once nothing more fits', () => {
        expect(isFull([{ question: q(4_000), answer: q(27_999) }])).toBe(false)
        expect(isFull([{ question: q(4_000), answer: q(28_000) }])).toBe(true)
    })
})

describe('mapAskError', () => {
    it('shows the wait from a 429', () => {
        expect(mapAskError(429, 'slow down — try again in 42s')).toEqual({
            message: 'Slow down — try again in 42s',
            retryable: true,
        })
    })

    it('shows a 422 reason without a retry', () => {
        expect(mapAskError(422, 'side chat is full')).toEqual({ message: 'side chat is full', retryable: false })
    })

    it('makes timeouts, server errors and network failures retryable', () => {
        expect(mapAskError(408, 'Agent response timeout').retryable).toBe(true)
        expect(mapAskError(503, 'x').retryable).toBe(true)
        expect(mapAskError(undefined, '').message).toMatch(/try again/)
    })
})
