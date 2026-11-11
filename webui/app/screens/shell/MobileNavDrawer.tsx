import { NavItem, SlideOver, UserMenu } from '@vizier/design'
import { useLocation, useNavigate } from 'react-router'
import { transport } from '../../api/transport'
import { hasBothVariants, useAppearance } from '../../stores/appearance'
import { useAgents } from '../../stores/agents'
import { useNav } from '../../stores/nav'
import { useNotifications } from '../../stores/notifications'
import { useSession } from '../../stores/session'
import { useWorkspaces } from '../../stores/workspaces'

/**
 * The phone's menu (spec 056): the app, not the agent. Chat, Home, Settings and Theme, and the
 * person at the foot. Everything about one agent is on its details page, reached from the top bar.
 */
export function MobileNavDrawer() {
    const navigate = useNavigate()
    const { pathname } = useLocation()
    const open = useNav((s) => s.open)
    const setOpen = useNav((s) => s.setOpen)
    const lastAgent = useNav((s) => s.lastAgent)
    const agents = useAgents((s) => s.agents)
    const { user, signOut } = useSession()
    const resetNotifications = useNotifications((s) => s.reset)
    const resetWorkspaces = useWorkspaces((s) => s.reset)
    const canSwitch = useAppearance((s) => hasBothVariants(s.paint))
    const toggleMode = useAppearance((s) => s.toggleMode)

    // Chat goes back to the agent the person was last on, else the first one there is.
    const chatAgent = lastAgent && agents.some((a) => a.agent_id === lastAgent) ? lastAgent : agents[0]?.agent_id
    const inChat = /^\/[^/]+\/(?:projects\/[^/]+\/)?chat(\/|$)/.test(pathname)

    return (
        <SlideOver open={open} side="left" title="Vizier" onClose={() => setOpen(false)} className="w-304!">
            <nav aria-label="Main" className="flex flex-col gap-2">
                {chatAgent && <NavItem icon="chat" label="Chat" href={`/${chatAgent}/chat`} active={inChat} />}
                <NavItem icon="home" label="Home" href="/" active={pathname === '/'} />
            </nav>
            <div className="h-px shrink-0 bg-border" />
            <div className="flex flex-col gap-2">
                <NavItem icon="settings" label="Settings" href="/settings/profile" active={pathname.startsWith('/settings')} />
                {canSwitch && <NavItem icon="theme" label="Theme" onSelect={toggleMode} />}
            </div>
            <div className="flex-1" />
            {/* The desktop owner is signed in by opening the app: the design's Sign out is hidden. */}
            <div className={transport.authMode === 'desktop' ? 'contents [&_button[aria-label="Sign out"]]:hidden' : 'contents'}>
                <UserMenu
                    name={user?.username ?? ''}
                    role={user?.role_name}
                    className="-mx-20 -mb-20 px-10"
                    onOpenProfile={() => navigate('/settings/profile')}
                    onSignOut={() => {
                        setOpen(false)
                        signOut()
                        resetNotifications()
                        resetWorkspaces()
                        navigate('/login')
                    }}
                />
            </div>
        </SlideOver>
    )
}
