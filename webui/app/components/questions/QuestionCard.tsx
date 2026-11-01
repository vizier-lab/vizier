import { useEffect, useMemo, useRef, useState } from 'react'
import { Button, Radio, TextField, Textarea } from '@vizier/design'
import type { QuestionAnswerPayload, QuestionRefusal, QuestionSetEntry } from '../../api/frames'
import { type Draft, initialDraft, missingQuestions, shownAnswer, toPayload } from '../../lib/questions'

export type {
    Question,
    QuestionAnswerPayload,
    QuestionOption,
    QuestionRefusal,
    QuestionSetEntry,
} from '../../api/frames'

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
    refusal,
}: {
    set: QuestionSetEntry
    onSubmit: (answer: QuestionAnswerPayload) => void
    onDecline: () => void
    /** The server's latest refusal of this set's answer; the card names the question and keeps what was typed. */
    refusal?: QuestionRefusal
}) {
    const [draft, setDraft] = useState<Draft>(() => initialDraft(set))
    const [errors, setErrors] = useState<Record<string, string>>({})
    const closed = set.state !== 'pending'
    const root = useRef<HTMLDivElement>(null)

    // The server's refusal lands on the question it names; one that names none is the card's own.
    const shownErrors = useMemo(() => {
        const merged = { ...errors }
        if (refusal?.questionId && !(refusal.questionId in merged)) merged[refusal.questionId] = refusal.message
        return merged
    }, [errors, refusal])
    const cardError = refusal && !refusal.questionId ? refusal.message : null

    // After a refused submission focus goes to the first problem, so a keyboard-only person lands
    // on what to fix instead of having to look for it (FR-064).
    const firstProblem = set.questions.find((q) => shownErrors[q.id])?.id
    useEffect(() => {
        if (!firstProblem) return
        const target = root.current?.querySelector<HTMLElement>(`[data-question="${CSS.escape(firstProblem)}"] :is(input, textarea, button)`)
        target?.focus()
    }, [firstProblem, refusal])

    const missing = useMemo(() => missingQuestions(set, draft), [draft, set])

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
        onSubmit(toPayload(set, draft))
    }

    return (
        <div
            ref={root}
            role="group"
            aria-label={set.title ?? 'Questions from the agent'}
            className="flex flex-col gap-14 rounded-lg border border-border bg-raised p-16"
        >
            <div className="flex flex-col gap-2">
                {set.title ? <h3 className="type-title text-strong">{set.title}</h3> : null}
                {set.description ? <p className="type-ui text-soft">{set.description}</p> : null}
            </div>

            {set.questions.map((question) => (
                <div key={question.id} data-question={question.id} className="flex flex-col gap-6">
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

                    {shownErrors[question.id] ? (
                        <p role="alert" className="type-ui text-danger">
                            {shownErrors[question.id]}
                        </p>
                    ) : null}
                </div>
            ))}

            {cardError ? (
                <p role="alert" className="type-ui text-danger">
                    {cardError}
                </p>
            ) : null}

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
