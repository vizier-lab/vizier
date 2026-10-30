import { Rail, UserMenu } from '@vizier/design'
import { useState } from 'react'
import { useNavigate } from 'react-router'
import { transport } from '../../api/transport'
import { agentState } from '../../api/endpoints/agents'
import { useAgents } from '../../stores/agents'
import { useNotifications } from '../../stores/notifications'
import { useSession } from '../../stores/session'
import { read, write } from '../../lib/remember'
import type { ShellView } from './pages'

const ORDER_KEY = 'vizier.rail.order'

/** The agents in the order this browser last arranged them; ones it hasn't seen yet follow, in their own order. */
function arranged<T extends { agentId: string }>(agents: T[], order: string[]): T[] {
    const rank = (id: string) => (order.includes(id) ? order.indexOf(id) : order.length)
    return [...agents].sort((a, b) => rank(a.agentId) - rank(b.agentId))
}

/** The narrow strip: Home, a tile for each agent, and at the foot the theme, settings and the person. */
export function ShellRail({ pathname, view, routeAgent }: { pathname: string; view: ShellView; routeAgent?: string }) {
    const navigate = useNavigate()
    const { user, toggleTheme, signOut, can } = useSession()
    const { agents, health } = useAgents()
    const resetNotifications = useNotifications((s) => s.reset)
    const [order, setOrder] = useState<string[]>(() => read(ORDER_KEY, []))

    return (
        <Rail
            homeHref="/"
            homeActive={pathname === '/'}
            onReorder={(ids) => {
                setOrder(ids)
                write(ORDER_KEY, ids)
            }}
            agents={arranged(
                agents.map((a) => ({
                    agentId: a.agent_id,
                    imageSrc: a.avatar_url || undefined,
                    name: a.name,
                    state: agentState(health[a.agent_id]),
                    href: `/${a.agent_id}/chat`,
                })),
                order
            )}
            activeAgentId={view === 'agent' ? routeAgent : undefined}
            addHref={can('all_agents:create') ? '/agents/new' : undefined}
            addActive={pathname === '/agents/new'}
            settingsHref="/settings/profile"
            settingsActive={view === 'settings'}
            onToggleTheme={toggleTheme}
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
                        navigate('/login')
                    }}
                />
                </div>
            }
        />
    )
}
