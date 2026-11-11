import { AgentRow, ListItem, SlideOver } from '@vizier/design'
import { useNavigate } from 'react-router'
import { agentState } from '../../api/endpoints/agents'
import type { WorkspaceSummary } from '../../api/endpoints/workspaces'
import { detailsPath } from '../../lib/agentDetails'
import { assetSrc } from '../../lib/fileUrls'
import { useAgents } from '../../stores/agents'
import { useNav } from '../../stores/nav'
import { useWorkspaces } from '../../stores/workspaces'
import { initialsOf } from './ShellRail'

/** Where a Workspace opens: its first topic, as the rail's tile does. */
export const workspaceHref = (w: WorkspaceSummary) => `/workspaces/${w.id}${w.topics[0] ? `/${w.topics[0].id}` : ''}`

/** A Workspace's tile: the starts of its name, as on the rail. */
export function WorkspaceTile({ name }: { name: string }) {
    return (
        <span aria-hidden className="grid size-34 shrink-0 place-items-center rounded-md bg-raised type-mono-xs text-muted">
            {initialsOf(name)}
        </span>
    )
}

/**
 * The phone's agent switcher (spec 056), a bottom sheet from the agent's name in the top bar:
 * the agents, then the Workspaces. Each agent row has its own details button, which opens that
 * agent's details without switching the chat first. Creating an agent is Home's job, not this.
 */
export function AgentSwitcherSheet({ current }: { current?: string }) {
    const navigate = useNavigate()
    const open = useNav((s) => s.switcherOpen)
    const setOpen = useNav((s) => s.setSwitcherOpen)
    const { agents, health } = useAgents()
    const workspaces = useWorkspaces((s) => s.list)
    const close = () => setOpen(false)
    const go = (href: string) => {
        close()
        navigate(href)
    }
    const sorted = [...agents].sort((a, b) => a.name.localeCompare(b.name))

    return (
        <SlideOver open={open} side="bottom" title="Switch agent" onClose={close}>
            <section aria-label="Agents" className="flex flex-col gap-2">
                <h3 className="px-4 pb-4 type-label uppercase text-faint">Agents</h3>
                {sorted.map((a) => (
                    <AgentRow
                        key={a.agent_id}
                        agentId={a.agent_id}
                        name={a.name}
                        imageSrc={assetSrc(a.avatar_url)}
                        state={agentState(health[a.agent_id])}
                        selected={a.agent_id === current}
                        chevron={false}
                        className="rounded-lg"
                        onSelect={() => go(`/${a.agent_id}/chat`)}
                        onDetails={() => go(detailsPath(a.agent_id))}
                    />
                ))}
            </section>
            {workspaces.length > 0 && (
                <section aria-label="Workspaces" className="flex flex-col gap-2 pt-8">
                    <h3 className="px-4 pb-4 type-label uppercase text-faint">Workspaces</h3>
                    {workspaces.map((w) => (
                        <ListItem
                            key={w.id}
                            shape="flush"
                            className="rounded-lg"
                            leading={<WorkspaceTile name={w.name} />}
                            title={<span className="type-body-strong">{w.name}</span>}
                            subtitle={`${w.topics.length} ${w.topics.length === 1 ? 'topic' : 'topics'}${w.working ? ' · working' : ''}`}
                            meta={w.unread > 0 ? `${w.unread} unread` : undefined}
                            onSelect={() => go(workspaceHref(w))}
                        />
                    ))}
                </section>
            )}
        </SlideOver>
    )
}
