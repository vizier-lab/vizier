import { AgentRow, ListItem, SectionHeading } from '@vizier/design'
import { useState } from 'react'
import { useLocation, useNavigate } from 'react-router'
import { agentState, type Agent, type AgentHealth } from '../../api/endpoints/agents'
import { assetSrc } from '../../lib/fileUrls'
import { relativeTime } from '../../lib/format'
import { useNav } from '../../stores/nav'
import { useSession } from '../../stores/session'
import { useWorkspaces } from '../../stores/workspaces'
import { withNewAgent } from '../agents/NewAgentDialog'
import { WorkspaceTile, workspaceHref } from '../shell/AgentSwitcherSheet'
import { NewWorkspaceDialog } from '../workspaces/NewWorkspaceDialog'

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
 * Home on a phone (spec 056): the agents and the Workspaces, each list with its + on the side.
 * No greeting and no activity panel, and no borders: the agent the person was last on is marked.
 */
export function MobileHome({ agents, health }: Props) {
    const navigate = useNavigate()
    const { pathname, search } = useLocation()
    const can = useSession((s) => s.can)
    const lastAgent = useNav((s) => s.lastAgent)
    const workspaces = useWorkspaces((s) => s.list)
    const [newWorkspace, setNewWorkspace] = useState(false)
    const mayAgent = can('all_agents:create')
    const mayWorkspace = can('workspaces:create')
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
            {(workspaces.length > 0 || mayWorkspace) && (
                <section aria-label="Workspaces" className="flex flex-col gap-2">
                    <SectionHeading
                        label="Workspaces"
                        className="px-4"
                        onAdd={mayWorkspace ? () => setNewWorkspace(true) : undefined}
                        addLabel="New workspace"
                    />
                    {workspaces.length === 0 ? (
                        <p className="px-4 py-8 type-small text-muted">No Workspaces yet.</p>
                    ) : (
                        workspaces.map((w) => (
                            <ListItem
                                key={w.id}
                                shape="flush"
                                className="rounded-lg"
                                leading={<WorkspaceTile name={w.name} />}
                                title={<span className="type-body-strong">{w.name}</span>}
                                subtitle={`${w.topics.length} ${w.topics.length === 1 ? 'topic' : 'topics'}${w.working ? ' · working' : ''}`}
                                meta={w.unread > 0 ? `${w.unread} unread` : undefined}
                                href={workspaceHref(w)}
                            />
                        ))
                    )}
                </section>
            )}
            <NewWorkspaceDialog open={newWorkspace} onClose={() => setNewWorkspace(false)} />
        </div>
    )
}
