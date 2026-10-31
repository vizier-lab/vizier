import { PlanCard as LibraryPlanCard } from '@vizier/design'
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
            summary={plan.summary}
            steps={plan.steps}
            building={building}
            onBuild={actionable ? onBuild : undefined}
        />
    )
}
