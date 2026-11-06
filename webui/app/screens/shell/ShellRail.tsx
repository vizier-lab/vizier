import { CreateChooser, Rail, UserMenu, type RailAgentPage, type RailWorkspace } from '@vizier/design'
import { nanoid } from 'nanoid'
import { useState } from 'react'
import { useLocation, useNavigate } from 'react-router'
import { transport } from '../../api/transport'
import { agentState } from '../../api/endpoints/agents'
import { useAgents } from '../../stores/agents'
import { useNotifications } from '../../stores/notifications'
import { hasBothVariants, useAppearance } from '../../stores/appearance'
import { useSession } from '../../stores/session'
import { canManageSecrets, canViewArtifacts } from '../../lib/access'
import { assetSrc } from '../../lib/fileUrls'
import { read, write } from '../../lib/remember'
import { useMobile } from '../../lib/useMobile'
import { useNav } from '../../stores/nav'
import { newAgentOpen, withNewAgent } from '../agents/NewAgentDialog'
import { NewWorkspaceDialog } from '../workspaces/NewWorkspaceDialog'
import { useWorkspaces } from '../../stores/workspaces'
import { PAGE_PATH, pageOf, type ShellView } from './pages'

const ORDER_KEY = 'vizier.rail.order'

/** Two letters for a Workspace tile: the starts of its first two words, else its first two letters. */
export function initialsOf(name: string): string {
    const words = name.split(/[^A-Za-z0-9]+/).filter(Boolean)
    const letters = words.length > 1 ? words[0][0] + words[1][0] : (words[0] ?? name).slice(0, 2)
    return letters.toUpperCase()
}

/** The agents in the order this browser last arranged them; ones it hasn't seen yet follow, in their own order. */
function arranged<T extends { agentId: string }>(agents: T[], order: string[]): T[] {
    const rank = (id: string) => (order.includes(id) ? order.indexOf(id) : order.length)
    return [...agents].sort((a, b) => rank(a.agentId) - rank(b.agentId))
}

/** The narrow strip: Home, a tile for each agent, and at the foot the theme, settings and the person. */
export function ShellRail({ pathname, view, routeAgent }: { pathname: string; view: ShellView; routeAgent?: string }) {
    const navigate = useNavigate()
    const { search } = useLocation()
    const { user, signOut, can, canEditAgent } = useSession()
    // Only a theme with both variants has anything to switch between.
    const canSwitch = useAppearance((s) => hasBothVariants(s.paint))
    const toggleMode = useAppearance((s) => s.toggleMode)
    const { agents, health, creating } = useAgents()
    const resetNotifications = useNotifications((s) => s.reset)
    const mobile = useMobile()
    const sidebarCollapsed = useNav((s) => s.sidebarCollapsed)
    const toggleSidebar = useNav((s) => s.toggleSidebar)
    const [order, setOrder] = useState<string[]>(() => read(ORDER_KEY, []))
    const workspaceList = useWorkspaces((s) => s.list)
    const resetWorkspaces = useWorkspaces((s) => s.reset)
    const [chooser, setChooser] = useState(false)
    const [newWorkspace, setNewWorkspace] = useState(false)
    const mayWorkspace = can('workspaces:create')
    const mayAgent = can('all_agents:create')
    const workspaces: RailWorkspace[] = workspaceList.map((w) => ({
        id: w.id,
        name: w.name,
        initials: initialsOf(w.name),
        href: `/workspaces/${w.id}${w.topics[0] ? `/${w.topics[0].id}` : ''}`,
        unread: w.unread || undefined,
        working: w.working,
    }))
    const activeWorkspaceId = view === 'workspace' ? pathname.split('/')[2] : undefined
    // A tile for each agent the server has, then one busy tile for each still being created.
    const tiles = arranged(
        agents.map((a) => ({
            agentId: a.agent_id,
            imageSrc: assetSrc(a.avatar_url),
            name: a.name,
            state: agentState(health[a.agent_id]),
            href: `/${a.agent_id}/chat`,
            // The same rules as the agent's own column and card.
            hiddenPages: [
                ...(canViewArtifacts(a) ? [] : (['artifacts'] as const)),
                ...(canManageSecrets(a) ? [] : (['secrets'] as const)),
            ],
            disabledPages: canEditAgent(a) ? [] : (['config'] as const),
        })),
        order
    )
    const page = pageOf(pathname)
    const pending = creating
        .filter((c) => !agents.some((a) => a.agent_id === c.agentId))
        .map((c) => ({
            agentId: c.agentId,
            name: c.name,
            state: 'starting' as const,
            href: `/${c.agentId}/chat`,
            pending: true,
        }))

    return (
        <>
        <Rail
            homeHref="/"
            homeActive={pathname === '/'}
            onReorder={(ids) => {
                setOrder(ids)
                write(ORDER_KEY, ids)
            }}
            workspaces={workspaces}
            activeWorkspaceId={activeWorkspaceId}
            agents={[...tiles, ...pending]}
            activeAgentId={view === 'agent' ? routeAgent : undefined}
            // One "+": a chooser when the person may make either, straight to the form when only one,
            // and none when neither.
            {...(mayWorkspace
                ? {
                      onCreate: () => (mayAgent ? setChooser(true) : setNewWorkspace(true)),
                      createLabel: mayAgent ? 'Create new' : 'New workspace',
                  }
                : mayAgent
                  ? { addHref: withNewAgent(`${pathname}${search}`) }
                  : {})}
            addActive={newAgentOpen(new URLSearchParams(search))}
            settingsHref="/settings/profile"
            settingsActive={view === 'settings'}
            onToggleTheme={canSwitch ? toggleMode : undefined}
            agentMenu={{
                activePage: page && page !== 'chat' ? page : undefined,
                onNew: (id) => navigate(`/${id}/chat/${nanoid(10)}`),
                onSelect: (id, p: RailAgentPage) => navigate(`/${id}${PAGE_PATH[p]}`),
            }}
            // Only an agent's column collapses, and a phone's drawer always shows it in full.
            sidebarToggle={
                !mobile && view === 'agent'
                    ? {
                          collapsed: sidebarCollapsed,
                          onToggle: toggleSidebar,
                          shortcut: /Mac|iPhone|iPad/.test(navigator.platform) ? '⌘B' : 'Ctrl B',
                      }
                    : undefined
            }
            user={
                // The desktop owner is signed in by opening the app: the design's Sign out is hidden.
                <div className={transport.authMode === 'desktop' ? 'contents [&_button[aria-label="Sign out"]]:hidden' : 'contents'}>
                <UserMenu
                    name={user?.username ?? ''}
                    role={user?.role_name}
                    collapsed
                    onOpenProfile={() => navigate('/settings/profile')}
                    onSignOut={() => {
                        signOut()
                        resetNotifications()
                        resetWorkspaces()
                        navigate('/login')
                    }}
                />
                </div>
            }
        />
        <CreateChooser
            open={chooser}
            onClose={() => setChooser(false)}
            onWorkspace={() => {
                setChooser(false)
                setNewWorkspace(true)
            }}
            onAgent={() => {
                setChooser(false)
                navigate(withNewAgent(`${pathname}${search}`))
            }}
        />
        <NewWorkspaceDialog open={newWorkspace} onClose={() => setNewWorkspace(false)} />
        </>
    )
}
