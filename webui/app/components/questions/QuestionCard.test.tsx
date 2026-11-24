import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import type { QuestionSetEntry } from '../../api/frames'
import { QuestionCard } from './QuestionCard'

const set = (extra: Partial<QuestionSetEntry> = {}): QuestionSetEntry => ({
    set_id: 's1',
    title: 'Project setup',
    description: 'Two quick choices.',
    state: 'pending',
    questions: [
        { id: 'name', prompt: 'Project name', type: 'text', help: 'Lowercase.' },
        { id: 'db', prompt: 'Database', type: 'choice', options: [{ value: 'pg', label: 'PostgreSQL' }, { value: 'sqlite', label: 'SQLite' }] },
        { id: 'note', prompt: 'Anything else?', type: 'text', optional: true },
    ],
    ...extra,
})

const draw = (s: QuestionSetEntry, refusal?: { code: string; message: string; questionId?: string }) =>
    renderToString(<QuestionCard set={s} onSubmit={() => {}} onDecline={() => {}} refusal={refusal} />)

describe('a pending question card', () => {
    it('shows every question in order with its prompt, help and required state', () => {
        const html = draw(set())
        expect(html.indexOf('Project name')).toBeLessThan(html.indexOf('Database'))
        expect(html.indexOf('Database')).toBeLessThan(html.indexOf('Anything else?'))
        expect(html).toContain('Lowercase.')
        expect(html).toContain('(optional)')
        expect(html).toContain('PostgreSQL')
        // One submit and one dismiss.
        expect(html).toContain('Submit')
        expect(html).toContain('Dismiss')
    })

    it('uses the agent\'s own labels for the two actions', () => {
        const html = draw(set({ submit_label: 'Create', dismiss_label: 'Cancel' }))
        expect(html).toContain('Create')
        expect(html).toContain('Cancel')
    })

    it('labels every field for a screen reader and groups the card', () => {
        const html = draw(set())
        expect(html).toContain('role="group"')
        expect(html).toContain('aria-label="Project setup"')
        // A text field is tied to its prompt.
        expect(html).toContain('for="q-s1-name"')
        expect(html).toContain('id="q-s1-name"')
    })

    it('puts the server\'s refusal on the question it names, or on the card when it names none', () => {
        const named = draw(set(), { code: 'invalid_answer', message: 'Not one of the options.', questionId: 'db' })
        expect(named).toContain('Not one of the options.')
        expect(named.match(/role="alert"/g)).toHaveLength(1)

        const general = draw(set(), { code: 'rate_limited', message: 'Slow down.' })
        expect(general).toContain('Slow down.')
    })
})

describe('a closed question card', () => {
    const answered = set({
        state: 'answered',
        answerer: 'alice',
        answers: [
            { question_id: 'name', state: 'answered', value: '<script>alert(1)</script> **bold** `x`' },
            { question_id: 'db', state: 'answered', value: 'pg', label: 'PostgreSQL' },
            { question_id: 'note', state: 'skipped' },
        ],
    })

    it('is a read-only record: every answer, skipped ones named, and nothing to submit again', () => {
        const html = draw(answered)
        expect(html).toContain('PostgreSQL')
        expect(html).toContain('skipped')
        expect(html).toContain('Answered by alice.')
        expect(html).not.toContain('Submit')
        expect(html).not.toContain('Dismiss')
        expect(html).not.toContain('<input')
        expect(html).not.toContain('<textarea')
    })

    it('draws an answer as text: markup is escaped and nothing runs', () => {
        const html = draw(answered)
        expect(html).not.toContain('<script>')
        expect(html).toContain('&lt;script&gt;alert(1)&lt;/script&gt;')
    })

    it('says so when it was dismissed or withdrawn', () => {
        expect(draw(set({ state: 'declined' }))).toContain('Dismissed.')
        expect(draw(set({ state: 'withdrawn' }))).toContain('Withdrawn.')
    })

    it('uses design tokens only: no literal colour in the card', () => {
        const html = draw(set())
        expect(html).not.toMatch(/#[0-9a-fA-F]{3,8}\b|rgb\(|hsl\(/)
    })
})
