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
    /** The agent that proposed it: its entity links point at its pages. */
    agentId?: string
}

/** A `propose_plan` call drawn in the reply: the plan and its one action (spec 041 US2). */
export function PlanCard({ plan, actionable, building, onBuild, agentId }: Props) {
    return (
        <LibraryPlanCard
            title={plan.title}
            summary={plan.summary ? <Markdown size="document" entities={agentId}>{plan.summary}</Markdown> : undefined}
            steps={plan.steps.map((step) => (
                <Markdown key={step} size="document" entities={agentId}>{step}</Markdown>
            ))}
            building={building}
            onBuild={actionable ? onBuild : undefined}
        />
    )
}
