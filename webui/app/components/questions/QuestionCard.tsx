import { useMemo, useState } from 'react'
import { Button, Callout, Radio, TextField, Textarea } from '@vizier/design'

/** One question as the server sends it (`specs/042-agent-asking-question`). */
export interface QuestionOption {
    value: string
    label: string
    description?: string
}

export interface Question {
    id: string
    prompt: string
    type: 'text' | 'choice' | 'multi_choice' | 'confirm'
    help?: string
    optional?: boolean
    default?: string | string[] | boolean
    options?: QuestionOption[]
    placeholder?: string
    max_length?: number
    multiline?: boolean
    min_selections?: number
    max_selections?: number
    yes_label?: string
    no_label?: string
}

export interface QuestionSetEntry {
    set_id: string
    title?: string
    description?: string
    questions: Question[]
    submit_label?: string
    dismiss_label?: string
    state: 'pending' | 'answered' | 'declined' | 'withdrawn'
    answers?: Array<{
        question_id: string
        state: 'answered' | 'skipped'
        value?: string | string[] | boolean
        label?: string
    }>
    answerer?: string
}

export interface QuestionAnswerPayload {
    set_id: string
    answers: Array<{ question_id: string; state: 'answered' | 'skipped'; value?: string | string[] | boolean }>
    declined: boolean
}

type Draft = Record<string, string | string[] | boolean | undefined>

function initialDraft(set: QuestionSetEntry): Draft {
    const draft: Draft = {}
    for (const question of set.questions) {
        if (question.default !== undefined) draft[question.id] = question.default
    }
    return draft
}

/** What a question's answer reads as, on a closed card. */
function shownAnswer(set: QuestionSetEntry, id: string): string {
    const answer = set.answers?.find((a) => a.question_id === id)
    if (!answer) return 'skipped'
    if (answer.state === 'skipped') return 'skipped'
    const value = answer.value
    if (answer.label) return answer.label
    if (Array.isArray(value)) return value.join(', ')
    if (typeof value === 'boolean') return value ? 'yes' : 'no'
    return value ?? 'skipped'
}

/**
 * A question set in the conversation (`specs/042-agent-asking-question` FR-011–FR-014, FR-020).
 *
 * Pending: every question in the agent's order, one submit and one dismiss. Answered, declined or
 * withdrawn: a read-only record of what was given, so a stale card can never be submitted again.
 * Validation lives here as well as on the server: the server is authoritative, and a refusal comes
 * back naming the question, which is what the inline error shows.
 */
export function QuestionCard({
    set,
    onSubmit,
    onDecline,
}: {
    set: QuestionSetEntry
    onSubmit: (answer: QuestionAnswerPayload) => void
    onDecline: () => void
}) {
    const [draft, setDraft] = useState<Draft>(() => initialDraft(set))
    const [errors, setErrors] = useState<Record<string, string>>({})
    const closed = set.state !== 'pending'

    const missing = useMemo(
        () =>
            set.questions
                .filter((q) => !q.optional && q.default === undefined)
                .filter((q) => {
                    const value = draft[q.id]
                    if (value === undefined) return true
                    if (typeof value === 'string') return value.trim().length === 0
                    if (Array.isArray(value)) return value.length === 0
                    return false
                })
                .map((q) => q.id),
        [draft, set.questions],
    )

    function put(id: string, value: string | string[] | boolean) {
        setDraft((current) => ({ ...current, [id]: value }))
        setErrors((current) => {
            const next = { ...current }
            delete next[id]
            return next
        })
    }

    function toggle(id: string, value: string, limit: number | undefined) {
        const current = draft[id]
        const chosen = Array.isArray(current) ? current : []
        if (chosen.includes(value)) {
            put(id, chosen.filter((v) => v !== value))
            return
        }
        if (limit !== undefined && chosen.length >= limit) {
            setErrors((current) => ({ ...current, [id]: `Pick at most ${limit}.` }))
            return
        }
        put(id, [...chosen, value])
    }

    function submit() {
        if (missing.length > 0) {
            // Nothing is sent: the person is told which question is missing and their answers stay.
            const next: Record<string, string> = {}
            for (const id of missing) next[id] = 'This question is required.'
            setErrors(next)
            return
        }
        onSubmit({
            set_id: set.set_id,
            declined: false,
            answers: set.questions.map((q) => {
                const value = draft[q.id]
                const skipped =
                    value === undefined ||
                    (typeof value === 'string' && value.trim().length === 0) ||
                    (Array.isArray(value) && value.length === 0)
                return skipped
                    ? { question_id: q.id, state: 'skipped' as const }
                    : { question_id: q.id, state: 'answered' as const, value }
            }),
        })
    }

    return (
        <div className="flex flex-col gap-14 rounded-lg border border-border bg-raised p-16">
            <div className="flex flex-col gap-2">
                {set.title ? <h3 className="type-title text-strong">{set.title}</h3> : null}
                {set.description ? <p className="type-ui text-soft">{set.description}</p> : null}
            </div>

            {set.questions.map((question) => (
                <div key={question.id} className="flex flex-col gap-6">
                    <label className="type-ui text-strong" htmlFor={`q-${set.set_id}-${question.id}`}>
                        {question.prompt}
                        {question.optional ? <span className="text-faint"> (optional)</span> : null}
                    </label>
                    {question.help ? <p className="type-ui text-faint">{question.help}</p> : null}

                    {closed ? (
                        <p className="type-code text-soft">{shownAnswer(set, question.id)}</p>
                    ) : question.type === 'text' ? (
                        question.multiline ? (
                            <Textarea
                                id={`q-${set.set_id}-${question.id}`}
                                value={typeof draft[question.id] === 'string' ? (draft[question.id] as string) : ''}
                                placeholder={question.placeholder}
                                onChange={(event) => put(question.id, event.target.value)}
                            />
                        ) : (
                            <TextField
                                id={`q-${set.set_id}-${question.id}`}
                                value={typeof draft[question.id] === 'string' ? (draft[question.id] as string) : ''}
                                placeholder={question.placeholder}
                                onChange={(event) => put(question.id, event.target.value)}
                            />
                        )
                    ) : question.type === 'confirm' ? (
                        <div className="flex gap-8">
                            <Button
                                variant={draft[question.id] === true ? 'primary' : 'secondary'}
                                onClick={() => put(question.id, true)}
                            >
                                {question.yes_label ?? 'Yes'}
                            </Button>
                            <Button
                                variant={draft[question.id] === false ? 'primary' : 'secondary'}
                                onClick={() => put(question.id, false)}
                            >
                                {question.no_label ?? 'No'}
                            </Button>
                        </div>
                    ) : (
                        <div className="flex flex-col gap-4">
                            {(question.options ?? []).map((option) =>
                                question.type === 'choice' ? (
                                    <Radio
                                        key={option.value}
                                        name={question.id}
                                        checked={draft[question.id] === option.value}
                                        onChange={() => put(question.id, option.value)}
                                        label={option.label}
                                    />
                                ) : (
                                    <label key={option.value} className="flex items-center gap-8 type-ui">
                                        <input
                                            type="checkbox"
                                            checked={
                                                Array.isArray(draft[question.id]) &&
                                                (draft[question.id] as string[]).includes(option.value)
                                            }
                                            onChange={() =>
                                                toggle(question.id, option.value, question.max_selections)
                                            }
                                        />
                                        {option.label}
                                    </label>
                                ),
                            )}
                        </div>
                    )}

                    {errors[question.id] ? (
                        <p role="alert" className="type-ui text-danger">
                            {errors[question.id]}
                        </p>
                    ) : null}
                </div>
            ))}

            {closed ? (
                <p className="type-ui text-faint">
                    {set.state === 'declined'
                        ? 'Dismissed.'
                        : set.state === 'withdrawn'
                          ? 'Withdrawn.'
                          : set.answerer
                            ? `Answered by ${set.answerer}.`
                            : 'Answered.'}
                </p>
            ) : (
                <div className="flex items-center gap-8">
                    <Button variant="primary" onClick={submit}>
                        {set.submit_label ?? 'Submit'}
                    </Button>
                    <Button variant="secondary" onClick={onDecline}>
                        {set.dismiss_label ?? 'Dismiss'}
                    </Button>
                </div>
            )}
        </div>
    )
}
