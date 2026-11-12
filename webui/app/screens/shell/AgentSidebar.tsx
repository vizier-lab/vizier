import {
    AgentMenu,
    ContextSidebar,
    HighlightMatch,
    Picker,
    PickerAction,
    SidebarAgent,
    type EssentialPage,
    type MenuPage,
} from '@vizier/design'
import { nanoid } from 'nanoid'
import { useEffect, useState } from 'react'
import { useLocation, useNavigate, useSearchParams } from 'react-router'
import { agentState } from '../../api/endpoints/agents'
import { canManageSecrets, canViewArtifacts } from '../../lib/access'
import { assetSrc } from '../../lib/fileUrls'
import { relativeTime } from '../../lib/format'
import { withNewAgent } from '../agents/NewAgentDialog'
import { useAgents } from '../../stores/agents'
import { useSession } from '../../stores/session'
import { AgentConversations } from './AgentConversations'
import { PAGE_PATH, pageOf } from './pages'

const ESSENTIAL = new Set<string>(['memory', 'artifacts', 'tasks', 'skills', 'dreams'])

/**
 * The agent's column: the agent card with its menu, the four essentials, and the conversations.
 * Everything here navigates; nothing here changes what a page shows.
 */
export function AgentSidebar({ agentId }: { agentId: string }) {
    const navigate = useNavigate()
    const { pathname } = useLocation()
    const [search] = useSearchParams()
    const { can, canEditAgent } = useSession()
    const { agents, health } = useAgents()
    const [query, setQuery] = useState('')
    const [switching, setSwitching] = useState(false)
    const agent = agents.find((a) => a.agent_id === agentId)
    const state = agent ? agentState(health[agentId]) : 'stopped'
    const page = pageOf(pathname)
    const newConversation = () => navigate(`/${agentId}/chat/${nanoid(10)}`)

    // Ctrl K (or ⌘K) opens the list of agents to switch to.
    useEffect(() => {
        const on = (e: KeyboardEvent) => {
            if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
                e.preventDefault()
                setSwitching(true)
            }
        }
        window.addEventListener('keydown', on)
        return () => window.removeEventListener('keydown', on)
    }, [])

    const q = query.trim().toLowerCase()
    const matching = agents
        .filter((a) => !q || a.name.toLowerCase().includes(q) || a.agent_id.toLowerCase().includes(q))
        .sort((a, b) => a.name.localeCompare(b.name))

    return (
        <ContextSidebar
            agent={
                <div className="relative">
                    <AgentMenu
                        trigger={
                            <SidebarAgent
                                agentId={agentId}
                                imageSrc={assetSrc(agent?.avatar_url)}
                                name={agent?.name ?? agentId}
                                state={state}
                                active={!!page && !ESSENTIAL.has(page) && page !== 'chat'}
                                detail={
                                    health[agentId]?.next_retry_at
                                        ? `retry ${relativeTime(health[agentId].next_retry_at!)}`
                                        : undefined
                                }
                            />
                        }
                        active={page && !ESSENTIAL.has(page) && page !== 'chat' ? (page as MenuPage) : undefined}
                        hiddenPages={agent && canManageSecrets(agent) ? undefined : ['secrets']}
                        disabledPages={agent && !canEditAgent(agent) ? ['config'] : undefined}
                        onSelect={(p) => navigate(`/${agentId}${PAGE_PATH[p]}`)}
                    />
                    {/* Out of the flow: it only anchors the agent list under the card. */}
                    <div className="absolute inset-x-0 bottom-0">
                    <Picker
                        label="Agents"
                        width={SIDEBAR_PICKER_WIDTH}
                        open={switching}
                        onOpenChange={setSwitching}
                        trigger={<span aria-hidden className="block h-0 w-full" />}
                        search={{ placeholder: 'Search agents', value: query, onChange: setQuery, total: `${agents.length}` }}
                        sections={[{ label: q ? undefined : 'All agents', items: matching }]}
                        getKey={(a) => a.agent_id}
                        itemHeight={52}
                        renderItem={(a, { query: hl }) => (
                            <div className="flex min-w-0 flex-col">
                                <span className="truncate type-body-strong text-fg">
                                    <HighlightMatch text={a.name} query={hl} />
                                </span>
                                <span className="truncate type-mono-xs text-faint">{a.agent_id}</span>
                            </div>
                        )}
                        onSelect={(a) => navigate(`/${a.agent_id}/chat`)}
                        footer={
                            can('all_agents:create') ? (
                                <PickerAction
                                    icon="plus"
                                    onClick={() => {
                                        const q = search.toString()
                                        navigate(withNewAgent(q ? `${pathname}?${q}` : pathname))
                                    }}
                                >
                                    Create agent
                                </PickerAction>
                            ) : undefined
                        }
                    />
                    </div>
                </div>
            }
            active={page && ESSENTIAL.has(page) ? (page as EssentialPage) : undefined}
            hiddenPages={agent && canViewArtifacts(agent) ? undefined : ['artifacts']}
            hrefFor={(p) => `/${agentId}${PAGE_PATH[p]}`}
            onNew={newConversation}
        >
            <AgentConversations agentId={agentId} />
        </ContextSidebar>
    )
}

const SIDEBAR_PICKER_WIDTH = 340
