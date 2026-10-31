import {
    AgentMenu,
    ContextSidebar,
    HighlightMatch,
    Picker,
    PickerAction,
    SessionList,
    SidebarAgent,
    type EssentialPage,
    type MenuPage,
} from '@vizier/design'
import { nanoid } from 'nanoid'
import { useEffect, useMemo, useState } from 'react'
import { useLocation, useNavigate, useSearchParams } from 'react-router'
import { agentState } from '../../api/endpoints/agents'
import { renameTopic } from '../../api/endpoints/chat'
import { toast } from '../../stores/toasts'
import { DeleteConversation, type DeleteTarget } from '../chat/DeleteConversation'
import { canManageSecrets, canViewArtifacts } from '../../lib/access'
import { conversationHref, conversationTitle, sidebarSections, sidebarTime } from '../../lib/conversations'
import { assetSrc } from '../../lib/fileUrls'
import { relativeTime } from '../../lib/format'
import { withNewAgent } from '../agents/NewAgentDialog'
import { ProjectsSection } from '../projects/ProjectsSection'
import { useAgents } from '../../stores/agents'
import { topicKey } from '../../stores/connection'
import { useSession } from '../../stores/session'
import { useTurns } from '../../stores/turns'
import { PAGE_PATH, pageOf } from './pages'
import { SESSIONS_STEP, useSessions } from './useSessions'

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
    const [shown, setShown] = useState(SESSIONS_STEP)
    const [sessionQuery, setSessionQuery] = useState('')
    // The conversation being deleted, while its confirmation is open; and a bump that makes the list look again.
    const [deleting, setDeleting] = useState<DeleteTarget | null>(null)
    const [listVersion, setListVersion] = useState(0)
    // The server searches; it is asked once typing pauses.
    const [sessionSearch, setSessionSearch] = useState('')
    useEffect(() => {
        const id = setTimeout(() => setSessionSearch(sessionQuery.trim()), 250)
        return () => clearTimeout(id)
    }, [sessionQuery])

    const agent = agents.find((a) => a.agent_id === agentId)
    const state = agent ? agentState(health[agentId]) : 'stopped'
    const page = pageOf(pathname)
    const { items, total } = useSessions(agentId, `${pathname}#${listVersion}`, shown, sessionSearch)

    const turns = useTurns((s) => s.topics)
    const isLive = (topic: string) => (turns[topicKey(agentId, topic)]?.phase ?? 'idle') !== 'idle'

    // The conversation on screen: a WebUI topic by id, a Discord or Telegram one by channel.
    const topicId = pathname.match(/^\/[^/]+\/chat\/([^/]+)$/)?.[1]
    const channel = pathname.endsWith('/chat/read') ? search.get('channel') : null
    const isCurrent = (c: (typeof items)[number]) =>
        channel ? c.channel === channel : c.kind === 'webui' && !!topicId && c.latest_topic === decodeURIComponent(topicId)

    const groups = useMemo(
        () =>
            sidebarSections(items).map((g) => ({
                label: g.label,
                items: g.items.map((c) => ({
                    key: `${c.channel}#${c.latest_topic}`,
                    title: conversationTitle(c.title, c.latest_topic),
                    time: sidebarTime(c.last_activity),
                    live: c.kind === 'webui' && isLive(c.latest_topic),
                    href: conversationHref(agentId, c),
                    active: isCurrent(c),
                    // Only the WebUI's own conversations: Discord and Telegram ones belong to those platforms.
                    deletable: c.kind === 'webui',
                    renamable: c.kind === 'webui',
                })),
            })),
        // eslint-disable-next-line react-hooks/exhaustive-deps
        [items, turns, topicId, channel, agentId]
    )

    const newConversation = () => navigate(`/${agentId}/chat/${nanoid(10)}`)

    useEffect(() => setShown(SESSIONS_STEP), [agentId, sessionSearch])
    useEffect(() => setSessionQuery(''), [agentId])

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
            <ProjectsSection agentId={agentId} />
            <SessionList
                groups={groups}
                onNew={newConversation}
                onDelete={(key) => {
                    const c = items.find((i) => `${i.channel}#${i.latest_topic}` === key)
                    if (c) setDeleting({ topic: c.latest_topic, title: conversationTitle(c.title, c.latest_topic) })
                }}
                onRename={async (key, title) => {
                    const c = items.find((i) => `${i.channel}#${i.latest_topic}` === key)
                    if (!c) return
                    try {
                        await renameTopic(agentId, c.latest_topic, title)
                        setListVersion((v) => v + 1)
                    } catch (e) {
                        toast.error("Couldn't rename the conversation", e instanceof Error ? e.message : undefined)
                    }
                }}
                search={{ value: sessionQuery, onChange: setSessionQuery }}
                empty={sessionSearch ? 'No conversation matched.' : undefined}
                showAll={
                    total > items.length
                        ? { label: 'Show more', onSelect: () => setShown((n) => n + SESSIONS_STEP) }
                        : undefined
                }
            />
            <DeleteConversation
                agentId={agentId}
                target={deleting}
                onClose={() => setDeleting(null)}
                onDeleted={(topic) => {
                    setListVersion((v) => v + 1)
                    if (topicId && decodeURIComponent(topicId) === topic) navigate(`/${agentId}/chat/${nanoid(10)}`)
                }}
            />
        </ContextSidebar>
    )
}

const SIDEBAR_PICKER_WIDTH = 340
