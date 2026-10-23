import {
    AgentSwitcher,
    HighlightMatch,
    Picker,
    PickerAction,
    IconButton,
    MobileTopBar,
    Sidebar,
    SlideOver,
    UserMenu,
    type SidebarPage,
} from '@vizier/design'
import { useEffect, useMemo, useState, type MouseEvent } from 'react'
import { Outlet, useLocation, useNavigate, useParams } from 'react-router'
import { agentState } from '../api/endpoints/agents'
import { relativeTime } from '../lib/format'
import { read, write } from '../lib/remember'
import { useMobile } from '../lib/useMobile'
import { useNav } from '../stores/nav'
import { useAgents } from '../stores/agents'
import { useSession } from '../stores/session'
import { ToastRegion } from './ToastRegion'

const COLLAPSED_KEY = 'vizier.sidebar.collapsed'
const RECENT_KEY = 'vizier.agents.recent'
const RECENT_MAX = 5

const PAGE_PATH: Record<SidebarPage, string> = {
    home: '',
    chat: '/chat',
    core: '/core',
    memory: '/memory',
    tasks: '/tasks',
    skills: '/skills',
    dreams: '/dream',
    channels: '/integrations/channels',
    addons: '/integrations/addons',
    mcp: '/integrations/mcp',
    usage: '/usage',
    config: '/config',
    settings: '/settings/profile',
}

function pageOf(pathname: string): SidebarPage | undefined {
    if (pathname === '/') return 'home'
    if (pathname.startsWith('/settings')) return 'settings'
    const rest = '/' + pathname.split('/').slice(2).join('/')
    const hit = (Object.entries(PAGE_PATH) as [SidebarPage, string][]).find(
        ([page, p]) =>
            p && page !== 'settings' && (rest === p || rest.startsWith(p + '/'))
    )
    return hit?.[0]
}

export default function AppShell() {
    const navigate = useNavigate()
    const { pathname } = useLocation()
    const params = useParams()
    const { user, toggleTheme, signOut, can } = useSession()
    const { agents, health, start } = useAgents()
    const [collapsed, setCollapsed] = useState(() => read(COLLAPSED_KEY, false))
    const [recent, setRecent] = useState<string[]>(() => read(RECENT_KEY, []))
    const [query, setQuery] = useState('')
    const mobile = useMobile()
    const navOpen = useNav((s) => s.open)
    const setNavOpen = useNav((s) => s.setOpen)

    useEffect(() => start(), [start])
    // Choosing a page closes the drawer.
    useEffect(() => setNavOpen(false), [pathname, setNavOpen])
    // A chat brings its own top bar, which has the topic and the agent's status.
    const ownsTopBar = /^\/[^/]+\/chat(\/|$)/.test(pathname)

    const routeAgent =
        params.agentId &&
        params.agentId !== 'settings' &&
        params.agentId !== 'agents'
            ? params.agentId
            : undefined
    const agentId =
        routeAgent ??
        recent.find((id) => agents.some((a) => a.agent_id === id)) ??
        agents[0]?.agent_id
    const agent = agents.find((a) => a.agent_id === agentId)

    useEffect(() => {
        if (!routeAgent) return
        setRecent((prev) => {
            const next = [
                routeAgent,
                ...prev.filter((id) => id !== routeAgent),
            ].slice(0, RECENT_MAX)
            write(RECENT_KEY, next)
            return next
        })
    }, [routeAgent])

    const stateOf = (id: string) => agentState(health[id])

    const sections = useMemo(() => {
        const q = query.trim().toLowerCase()
        const match = (a: (typeof agents)[number]) =>
            !q ||
            a.name.toLowerCase().includes(q) ||
            a.agent_id.toLowerCase().includes(q)
        const recents = recent
            .map((id) => agents.find((a) => a.agent_id === id))
            .filter((a) => a && match(a)) as typeof agents
        const rest = [...agents]
            .filter(match)
            .sort((a, b) => a.name.localeCompare(b.name))
        return q
            ? [{ items: rest }]
            : [
                  ...(recents.length
                      ? [{ label: 'Recent', items: recents }]
                      : []),
                  { label: 'All agents', items: rest },
              ]
    }, [agents, recent, query])

    // The shell's anchors are plain hrefs; take them over so a click doesn't reload the app.
    const onClick = (e: MouseEvent) => {
        if (
            e.defaultPrevented ||
            e.button !== 0 ||
            e.metaKey ||
            e.ctrlKey ||
            e.shiftKey
        )
            return
        const a = (e.target as HTMLElement).closest('a')
        const href = a?.getAttribute('href')
        if (!a || !href?.startsWith('/') || a.target === '_blank') return
        e.preventDefault()
        navigate(href)
    }

    const switcher = (
        <Picker
            label="Agents"
            trigger={
                <AgentSwitcher
                    agentId={agent?.agent_id ?? 'none'}
                    name={agent?.name ?? 'Choose an agent'}
                    state={agent ? stateOf(agent.agent_id) : 'stopped'}
                    detail={
                        health[agent?.agent_id ?? '']?.next_retry_at
                            ? `retry ${relativeTime(health[agent!.agent_id].next_retry_at!)}`
                            : undefined
                    }
                    collapsed={collapsed}
                />
            }
            search={{
                placeholder: 'Search agents',
                value: query,
                onChange: setQuery,
                total: `${agents.length}`,
            }}
            sections={sections}
            getKey={(a) => a.agent_id}
            itemHeight={52}
            renderItem={(a, { query }) => (
                <div className="flex min-w-0 flex-col">
                    <span className="truncate type-body-strong text-fg">
                        <HighlightMatch text={a.name} query={query} />
                    </span>
                    <span className="truncate type-mono-xs text-faint">
                        {a.agent_id}
                    </span>
                </div>
            )}
            onSelect={(a) => navigate(`/${a.agent_id}/chat`)}
            footer={
                can('all_agents:create') ? (
                    <PickerAction
                        icon="plus"
                        onClick={() => navigate('/agents/new')}
                    >
                        Create agent
                    </PickerAction>
                ) : undefined
            }
        />
    )

    const sidebar = (inDrawer: boolean) => (
        <Sidebar
            agent={switcher}
            user={
                <UserMenu
                    name={user?.username ?? ''}
                    role={user?.role_name}
                    collapsed={inDrawer ? false : collapsed}
                    onOpenProfile={() => navigate('/settings/profile')}
                    onSignOut={() => {
                        signOut()
                        navigate('/login')
                    }}
                />
            }
            active={pageOf(pathname)}
            collapsed={inDrawer ? false : collapsed}
            className={inDrawer ? 'w-full! border-r-0!' : undefined}
            hrefFor={(page) =>
                page === 'home'
                    ? '/'
                    : page === 'settings'
                      ? '/settings/profile'
                      : `/${agentId ?? ''}${PAGE_PATH[page]}`
            }
            onToggleTheme={toggleTheme}
            onToggleCollapsed={() => {
                setCollapsed((c) => {
                    write(COLLAPSED_KEY, !c)
                    return !c
                })
            }}
        />
    )

    if (mobile) {
        return (
            <div
                className="flex h-dvh min-w-0 flex-col bg-ground text-fg"
                onClick={onClick}
            >
                {!ownsTopBar && (
                    <MobileTopBar
                        onMenu={() => setNavOpen(true)}
                        trailing={
                            pathname === '/' && can('all_agents:create') ? (
                                <IconButton
                                    icon="plus"
                                    aria-label="New agent"
                                    onClick={() => navigate('/agents/new')}
                                />
                            ) : (
                                <span className="size-36" aria-hidden />
                            )
                        }
                    />
                )}
                <main className="flex min-h-0 min-w-0 flex-1 flex-col">
                    <Outlet />
                </main>
                <SlideOver
                    open={navOpen}
                    title="Menu"
                    onClose={() => setNavOpen(false)}
                >
                    <div className="-m-20 h-[calc(100%+40px)]">
                        {sidebar(true)}
                    </div>
                </SlideOver>
                <ToastRegion />
            </div>
        )
    }

    return (
        <div className="flex h-screen bg-ground text-fg" onClick={onClick}>
            {sidebar(false)}
            <main className="flex min-w-0 flex-1 flex-col">
                <Outlet />
            </main>
            <ToastRegion />
        </div>
    )
}
