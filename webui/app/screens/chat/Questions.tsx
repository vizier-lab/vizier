import type { ReactNode } from 'react'
import type { QuestionAnswerPayload, QuestionRefusal, Turn } from '../../api/frames'
import { QuestionCard } from '../../components/questions/QuestionCard'

/**
 * The question sets of one turn, drawn as cards after the text the agent wrote with them
 * (`specs/042-agent-asking-question`). A card is the stored history entry: its state arrives
 * filled by the server, so it is the same after a reload, in a second tab and after a restart.
 */
export function renderQuestions(
    turn: Turn,
    refusals: Record<string, QuestionRefusal>,
    answer: (answer: QuestionAnswerPayload) => void
): ReactNode {
    const sets = turn.questions.flatMap((entry) => (entry.content.QuestionSet ? [entry.content.QuestionSet] : []))
    if (sets.length === 0) return null
    return (
        <div className="flex flex-col gap-10">
            {sets.map((set) => (
                <QuestionCard
                    key={set.set_id}
                    set={set}
                    refusal={refusals[set.set_id]}
                    onSubmit={answer}
                    onDecline={() => answer({ set_id: set.set_id, answers: [], declined: true })}
                />
            ))}
        </div>
    )
}
