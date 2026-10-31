import { Button, Card } from '@vizier/design'
import type { Plan } from '../../../lib/plans'

type Props = {
    plan: Plan
    /** False once a later message exists: the card becomes history. */
    actionable: boolean
    onBuild: () => void
    onKeepPlanning: () => void
}

/** A `propose_plan` call drawn in the reply: the plan and its one-action build (spec 041 US2). */
export function PlanCard({ plan, actionable, onBuild, onKeepPlanning }: Props) {
    return (
        <Card title={plan.title} description={plan.summary} level={3}>
            <ol className="flex list-decimal flex-col gap-6 pl-20 type-body text-soft">
                {plan.steps.map((step, i) => (
                    <li key={i}>{step}</li>
                ))}
            </ol>
            {actionable && (
                <div className="flex flex-wrap items-center gap-8">
                    <Button variant="primary" icon="play" onClick={onBuild}>
                        Build this plan
                    </Button>
                    <Button onClick={onKeepPlanning}>Keep planning</Button>
                </div>
            )}
        </Card>
    )
}
