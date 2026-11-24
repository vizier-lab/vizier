import { PlanCard as LibraryPlanCard } from '@vizier/design'
import { Markdown } from '../../shared/Markdown'
import type { Plan } from '../../../lib/plans'

type Props = {
    plan: Plan
    /** False once a later message exists: the card becomes history. */
    actionable: boolean
    /** Build was pressed and the chat is switching mode. */
    building?: boolean
    onBuild: () => void
}

/** A `propose_plan` call drawn in the reply: the plan and its one action (spec 041 US2). */
export function PlanCard({ plan, actionable, building, onBuild }: Props) {
    return (
        <LibraryPlanCard
            title={plan.title}
            summary={plan.summary ? <Markdown size="document">{plan.summary}</Markdown> : undefined}
            steps={plan.steps.map((step) => (
                <Markdown key={step} size="document">{step}</Markdown>
            ))}
            building={building}
            onBuild={actionable ? onBuild : undefined}
        />
    )
}
