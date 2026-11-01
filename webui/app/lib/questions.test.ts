import { describe, expect, it } from 'vitest'
import type { Question, QuestionSetEntry } from '../api/frames'
import { declinePayload, initialDraft, isBlank, missingQuestions, shownAnswer, toPayload } from './questions'

const q = (id: string, extra: Partial<Question> = {}): Question => ({ id, prompt: id, type: 'text', ...extra })
const set = (questions: Question[], extra: Partial<QuestionSetEntry> = {}): QuestionSetEntry => ({
    set_id: 's1',
    questions,
    state: 'pending',
    ...extra,
})

describe('what a question card submits', () => {
    it('blocks a required question left blank and names it, whitespace included', () => {
        const s = set([q('name'), q('db', { type: 'choice' }), q('note', { optional: true })])
        expect(missingQuestions(s, {})).toEqual(['name', 'db'])
        expect(missingQuestions(s, { name: '   ', db: 'pg' })).toEqual(['name'])
        expect(missingQuestions(s, { name: 'demo', db: 'pg' })).toEqual([])
        // A choose-several with nothing ticked is blank; a confirm answered "no" is not.
        expect(isBlank([])).toBe(true)
        expect(isBlank(false)).toBe(false)
    })

    it('counts a default as an answer, so it neither blocks nor needs typing', () => {
        const s = set([q('name', { default: 'demo' })])
        expect(initialDraft(s)).toEqual({ name: 'demo' })
        expect(missingQuestions(s, initialDraft(s))).toEqual([])
    })

    it('sends every question exactly once, an optional blank one as an explicit skip', () => {
        const s = set([q('name'), q('note', { optional: true }), q('go', { type: 'confirm' })])
        expect(toPayload(s, { name: 'demo', note: '  ', go: false })).toEqual({
            set_id: 's1',
            declined: false,
            answers: [
                { question_id: 'name', state: 'answered', value: 'demo' },
                { question_id: 'note', state: 'skipped' },
                { question_id: 'go', state: 'answered', value: false },
            ],
        })
    })

    it('keeps markup, script and command text exactly as typed: it is data', () => {
        const hostile = '<script>alert(1)</script> $(rm -rf /) `id`'
        const payload = toPayload(set([q('name')]), { name: hostile })
        expect(payload.answers[0].value).toBe(hostile)
    })

    it('dismisses the whole set with no answers', () => {
        expect(declinePayload(set([q('a')]))).toEqual({ set_id: 's1', answers: [], declined: true })
    })

    it('reads each answer back on a closed card, skipped ones included', () => {
        const s = set([q('a'), q('b'), q('c', { type: 'multi_choice' }), q('d', { type: 'confirm' }), q('e')], {
            state: 'answered',
            answers: [
                { question_id: 'a', state: 'answered', value: 'pg', label: 'PostgreSQL' },
                { question_id: 'b', state: 'skipped' },
                { question_id: 'c', state: 'answered', value: ['x', 'y'] },
                { question_id: 'd', state: 'answered', value: true },
            ],
        })
        expect(['a', 'b', 'c', 'd', 'e'].map((id) => shownAnswer(s, id))).toEqual([
            'PostgreSQL',
            'skipped',
            'x, y',
            'yes',
            'skipped',
        ])
    })
})
