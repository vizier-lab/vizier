import { Rail, UserMenu } from '@vizier/design'
import { useNavigate } from 'react-router'
import { agentState } from '../../api/endpoints/agents'
import { useAgents } from '../../stores/agents'
import { useNotifications } from '../../stores/notifications'
import { useSession } from '../../stores/session'
import type { ShellView } from './pages'

/** The narrow strip: Home, a tile for each agent, and at the foot the theme, settings and the person. */
export function ShellRail({ pathname, view, routeAgent }: { pathname: string; view: ShellView; routeAgent?: string }) {
    const navigate = useNavigate()
    const { user, toggleTheme, signOut, can } = useSession()
    const { agents, health } = useAgents()
    const resetNotifications = useNotifications((s) => s.reset)

    return (
        <Rail
            homeHref="/"
            homeActive={pathname === '/'}
            agents={agents.map((a) => ({
                agentId: a.agent_id,
                name: a.name,
                state: agentState(health[a.agent_id]),
                href: `/${a.agent_id}/chat`,
            }))}
            activeAgentId={view === 'agent' ? routeAgent : undefined}
            addHref={can('all_agents:create') ? '/agents/new' : undefined}
            addActive={pathname === '/agents/new'}
            settingsHref="/settings/profile"
            settingsActive={view === 'settings'}
            onToggleTheme={toggleTheme}
            user={
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
            }
        />
    )
}
