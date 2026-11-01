// What a question card does with a person's answers, apart from drawing them
// (specs/042-agent-asking-question FR-011–FR-014). Pure, so it is tested without a browser.

import type { QuestionAnswerPayload, QuestionSetEntry } from '../api/frames'

export type DraftValue = string | string[] | boolean | undefined
export type Draft = Record<string, DraftValue>

/** The answers a question starts with: its default, which counts as an answer. */
export function initialDraft(set: QuestionSetEntry): Draft {
    const draft: Draft = {}
    for (const question of set.questions) {
        if (question.default !== undefined) draft[question.id] = question.default
    }
    return draft
}

/** Nothing in it, or only whitespace: the same thing the server counts as unanswered. */
export function isBlank(value: DraftValue): boolean {
    if (value === undefined) return true
    if (typeof value === 'string') return value.trim().length === 0
    if (Array.isArray(value)) return value.length === 0
    return false
}

/** The required questions with no answer. A required question left blank blocks the whole submission. */
export function missingQuestions(set: QuestionSetEntry, draft: Draft): string[] {
    return set.questions
        .filter((question) => !question.optional && question.default === undefined)
        .filter((question) => isBlank(draft[question.id]))
        .map((question) => question.id)
}

/** The submission: every question exactly once, an optional blank one as an explicit `skipped`. */
export function toPayload(set: QuestionSetEntry, draft: Draft): QuestionAnswerPayload {
    return {
        set_id: set.set_id,
        declined: false,
        answers: set.questions.map((question) => {
            const value = draft[question.id]
            return isBlank(value)
                ? { question_id: question.id, state: 'skipped' as const }
                : { question_id: question.id, state: 'answered' as const, value: value as string | string[] | boolean }
        }),
    }
}

/** A dismissal: no answers, and the whole set declined. */
export const declinePayload = (set: QuestionSetEntry): QuestionAnswerPayload => ({
    set_id: set.set_id,
    answers: [],
    declined: true,
})

/** What one question's answer reads as on a closed card. */
export function shownAnswer(set: QuestionSetEntry, id: string): string {
    const answer = set.answers?.find((a) => a.question_id === id)
    if (!answer || answer.state === 'skipped') return 'skipped'
    if (answer.label) return answer.label
    const value = answer.value
    if (Array.isArray(value)) return value.join(', ')
    if (typeof value === 'boolean') return value ? 'yes' : 'no'
    return value ?? 'skipped'
}
