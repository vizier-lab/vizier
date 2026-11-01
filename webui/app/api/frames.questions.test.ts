import { describe, expect, it } from 'vitest'
import { classifyFrame } from './frames'

describe('the question frames', () => {
    it('tells a question_set update from a response', () => {
        const frame = classifyFrame({ question_set: { set_id: 's', state: 'answered', answers: [], answerer: 'alice' } })
        expect(frame).toEqual({ kind: 'question_set', update: { set_id: 's', state: 'answered', answers: [], answerer: 'alice' } })
    })

    it('tells a sensitive ask and its closing apart', () => {
        const ask = { ask_id: 'a', purpose: 'p', protected: true, min_bytes: 8, max_bytes: 10 }
        expect(classifyFrame({ sensitive_ask: ask })).toEqual({ kind: 'sensitive_ask', ask })
        expect(classifyFrame({ sensitive_ask_closed: { ask_id: 'a', outcome: 'declined' } })).toEqual({
            kind: 'sensitive_ask_closed',
            askId: 'a',
            outcome: 'declined',
        })
    })

    it('still ignores a frame it does not know (additive protocol)', () => {
        expect(classifyFrame({ something_new: { a: 1 } })).toBeNull()
    })
})
