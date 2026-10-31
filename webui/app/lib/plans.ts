import type { Turn } from '../api/frames'

/** A plan proposed by `propose_plan` in a Plan-mode project chat (spec 041 US2). */
export type Plan = {
    title: string
    summary?: string
    steps: string[]
}

/** The plan a turn proposed, from the `execute_python` report that ended it. */
export function planOf(turn: Turn): Plan | null {
    for (let i = turn.trail.length - 1; i >= 0; i--) {
        const event = turn.trail[i]
        if (event.kind !== 'python' || !event.report?.ok) continue
        const call = [...event.report.tool_calls]
            .reverse()
            .find((c) => c.name === 'propose_plan' && c.ok)
        if (!call?.arguments) continue
        const title = call.arguments.title
        const steps = call.arguments.steps
        if (typeof title !== 'string' || !Array.isArray(steps)) return null
        const summary = call.arguments.summary
        return {
            title,
            summary: typeof summary === 'string' ? summary : undefined,
            steps: steps.filter((step): step is string => typeof step === 'string'),
        }
    }
    return null
}

/** The plan's actions belong to the newest turn; an older plan is history. */
export function planActionsVisible(turnKey: string, newestKey: string | null): boolean {
    return newestKey !== null && turnKey === newestKey
}
