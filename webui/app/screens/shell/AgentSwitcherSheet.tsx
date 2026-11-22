import { AgentRow, SlideOver } from '@vizier/design'
import { useNavigate } from 'react-router'
import { agentState } from '../../api/endpoints/agents'
import { detailsPath } from '../../lib/agentDetails'
import { assetSrc } from '../../lib/fileUrls'
import { useAgents } from '../../stores/agents'
import { useNav } from '../../stores/nav'

/**
 * The phone's agent switcher (spec 056), a bottom sheet from the agent's name in the top bar:
 * the agents. Each agent row has its own details button, which opens that
 * agent's details without switching the chat first. Creating an agent is Home's job, not this.
 */
export function AgentSwitcherSheet({ current }: { current?: string }) {
    const navigate = useNavigate()
    const open = useNav((s) => s.switcherOpen)
    const setOpen = useNav((s) => s.setSwitcherOpen)
    const { agents, health } = useAgents()
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
        </SlideOver>
    )
}
