import { AgentRow, PageHeader } from '@vizier/design'
import { useNavigate } from 'react-router'
import { agentState, type Agent, type AgentHealth } from '../../api/endpoints/agents'
import type { Overview } from '../../api/endpoints/overview'
import { greetingMeta } from '../../lib/overview'
import { relativeTime } from '../../lib/format'
import { ActivityPanel } from './ActivityPanel'

type Row = Agent & { last_active?: string }

type Props = {
    greeting: string
    overview?: Overview
    agents: Row[]
    health: Record<string, AgentHealth>
    total: number
    onShowAll: () => void
}

const stateOf = agentState

function detailOf(
    state: ReturnType<typeof stateOf>,
    a: Row,
    h?: AgentHealth
): string {
    if (state === 'retrying' && h?.next_retry_at)
        return `retry ${relativeTime(h.next_retry_at)}`
    if (state === 'failed') return 'failed'
    return a.last_active ? relativeTime(a.last_active) : 'never active'
}

/** Home on a phone: greeting, a compact day, and the court as one list. */
export function MobileHome({
    greeting,
    overview,
    agents,
    health,
    total,
    onShowAll,
}: Props) {
    const navigate = useNavigate()
    return (
        <div className="flex min-h-0 flex-1 flex-col gap-20 overflow-y-auto px-16 py-20">
            <PageHeader
                title={greeting}
                meta={overview ? greetingMeta(overview) : undefined}
            />
            <ActivityPanel compact />
            <section aria-label="The court" className="flex flex-col gap-12">
                <h2 className="type-heading text-fg">The court</h2>
                <div className="flex flex-col divide-y divide-border rounded-xl border border-border bg-surface">
                    {agents.map((a) => {
                        const h = health[a.agent_id]
                        const state = stateOf(h)
                        return (
                            <AgentRow
                                key={a.agent_id}
                                agentId={a.agent_id}
                                name={a.name}
                                state={state}
                                detail={detailOf(state, a, h)}
                                shape="flush"
                                href={`/${a.agent_id}/chat`}
                                onSelect={() => navigate(`/${a.agent_id}/chat`)}
                            />
                        )
                    })}
                </div>
                {total > agents.length && (
                    <button
                        type="button"
                        onClick={onShowAll}
                        className="rounded-lg border border-dashed border-border-strong px-16 py-14 type-ui text-muted"
                    >
                        Show all {total.toLocaleString('en-US')} agents
                    </button>
                )}
            </section>
        </div>
    )
}
