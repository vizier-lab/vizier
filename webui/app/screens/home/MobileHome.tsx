import { AgentRow, SectionHeading } from '@vizier/design'
import { useLocation, useNavigate } from 'react-router'
import { agentState, type Agent, type AgentHealth } from '../../api/endpoints/agents'
import { assetSrc } from '../../lib/fileUrls'
import { relativeTime } from '../../lib/format'
import { useNav } from '../../stores/nav'
import { useSession } from '../../stores/session'
import { withNewAgent } from '../agents/NewAgentDialog'

type Props = {
    agents: Agent[]
    health: Record<string, AgentHealth>
}

/** A row's second line: how it is doing when that is news, else nothing beyond its state. */
function detailOf(h?: AgentHealth): string | undefined {
    if (h?.next_retry_at) return `retry ${relativeTime(h.next_retry_at)}`
    return undefined
}

/**
 * Home on a phone (spec 056): the agents, with its + on the side.
 * No greeting and no activity panel, and no borders: the agent the person was last on is marked.
 */
export function MobileHome({ agents, health }: Props) {
    const navigate = useNavigate()
    const { pathname, search } = useLocation()
    const can = useSession((s) => s.can)
    const lastAgent = useNav((s) => s.lastAgent)
    const mayAgent = can('all_agents:create')
    const sorted = [...agents].sort((a, b) => a.name.localeCompare(b.name))

    return (
        <div className="flex min-h-0 flex-1 flex-col gap-20 overflow-y-auto px-12 pt-8 pb-24">
            <section aria-label="Agents" className="flex flex-col gap-2">
                <SectionHeading
                    label="Agents"
                    className="px-4"
                    onAdd={mayAgent ? () => navigate(withNewAgent(`${pathname}${search}`)) : undefined}
                    addLabel="New agent"
                />
                {sorted.map((a) => (
                    <AgentRow
                        key={a.agent_id}
                        agentId={a.agent_id}
                        name={a.name}
                        imageSrc={assetSrc(a.avatar_url)}
                        state={agentState(health[a.agent_id])}
                        detail={detailOf(health[a.agent_id])}
                        selected={a.agent_id === lastAgent}
                        className="rounded-lg"
                        href={`/${a.agent_id}/chat`}
                    />
                ))}
            </section>
        </div>
    )
}
