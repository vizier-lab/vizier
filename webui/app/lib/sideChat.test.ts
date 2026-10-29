import { describe, expect, it } from 'vitest'
import { initialSideChat, mapAskError, MAX_QUESTION_CHARS, refusal, sideChatReducer } from './sideChat'

describe('sideChatReducer', () => {
    it('asks and answers the one question', () => {
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
        s = sideChatReducer(s, { type: 'answered', index: 0, answer: 'late' })
        expect(s.turns[0].answer).toBe('a')
    })

    it('reset clears everything', () => {
        const s = sideChatReducer(initialSideChat, { type: 'ask', question: 'q' })
        expect(sideChatReducer(s, { type: 'reset' })).toEqual(initialSideChat)
    })
})

describe('refusal', () => {
    it('refuses an empty question and one over the limit', () => {
        expect(refusal('   ')).toMatch(/Type a question/)
        expect(refusal('a'.repeat(MAX_QUESTION_CHARS + 1))).toMatch(/4,000/)
    })

    it('accepts a question at the limit', () => {
        expect(refusal('a'.repeat(MAX_QUESTION_CHARS))).toBeNull()
        expect(refusal('hi')).toBeNull()
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
