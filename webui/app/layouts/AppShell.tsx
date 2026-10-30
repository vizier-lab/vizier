import { IconButton, MobileTopBar, ShellFrame, SlideOver } from '@vizier/design'
import { useEffect, useState, type MouseEvent } from 'react'
import { Outlet, useLocation, useMatches, useNavigate, useParams } from 'react-router'
import { motion } from 'motion/react'
import { motionDurations, motionEases } from '@vizier/design'
import { useMobile } from '../lib/useMobile'
import { useNav } from '../stores/nav'
import { useAgents } from '../stores/agents'
import { useNotifications } from '../stores/notifications'
import { NotificationsAside, NotificationsPanel } from '../screens/notifications/NotificationsPanel'
import { NotificationsBell } from '../screens/shared/NotificationsBell'
import { NotificationToasts } from '../screens/shared/NotificationToasts'
import { SettingsSidebar } from '../screens/settings/SettingsSidebar'
import { AgentSidebar } from '../screens/shell/AgentSidebar'
import { viewOf } from '../screens/shell/pages'
import { ShellRail } from '../screens/shell/ShellRail'
import { useSession } from '../stores/session'
import { ToastRegion } from './ToastRegion'

export default function AppShell() {
    const navigate = useNavigate()
    const { pathname } = useLocation()
    const params = useParams()
    const matches = useMatches()
    const { can } = useSession()
    const start = useAgents((s) => s.start)
    const mobile = useMobile()
    const notificationsOpen = useNotifications((s) => s.open)
    const navOpen = useNav((s) => s.open)
    const setNavOpen = useNav((s) => s.setOpen)
    const resetNotifications = useNotifications((s) => s.reset)
    const setViewing = useNotifications((s) => s.setViewing)
    const [drawerAgent, setDrawerAgent] = useState<string>()

    useEffect(() => start(), [start])
    // A fresh session starts from zero, then the badge is kept current by the
    // store's own 15s poll.
    useEffect(() => {
        resetNotifications()
        return useNotifications.getState().start()
    }, [resetNotifications])
    // A reply to the conversation on screen is read on arrival, not toasted.
    useEffect(() => setViewing(pathname), [pathname, setViewing])
    // Choosing a page closes the drawer.
    useEffect(() => setNavOpen(false), [pathname, setNavOpen])
    // A chat brings its own top bar, which has the topic and the agent's status.
    const ownsTopBar = /^\/[^/]+\/chat(\/|$)/.test(pathname)

    const routeAgent =
        params.agentId && params.agentId !== 'settings' && params.agentId !== 'agents'
            ? params.agentId
            : undefined
    const view = viewOf(pathname, routeAgent)
    // The drawer on a phone keeps showing the last agent while you are on Home or Settings.
    useEffect(() => {
        if (routeAgent) setDrawerAgent(routeAgent)
    }, [routeAgent])

    // Which screen is showing, rather than which URL: a screen keeps its
    // identity while its parameters change, so switching conversation or
    // document doesn't re-animate the app around it.
    const screen = matches[matches.length - 1]?.id ?? pathname
    // The settings pages share one frame: moving between them must not
    // re-animate the menu beside them.
    const routeKey = pathname.startsWith('/settings') ? 'settings' : screen

    // The shell's anchors are plain hrefs; take them over so a click doesn't reload the app.
    const onClick = (e: MouseEvent) => {
        if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey) return
        const a = (e.target as HTMLElement).closest('a')
        const href = a?.getAttribute('href')
        if (!a || !href?.startsWith('/') || a.target === '_blank') return
        e.preventDefault()
        navigate(href)
    }

    const admin = can('settings:providers') || can('users:manage') || can('roles:manage')
    const settingsPage = pathname.split('/')[2] ?? 'profile'
    const sidebar =
        view === 'agent' && routeAgent ? (
            <AgentSidebar agentId={routeAgent} />
        ) : view === 'settings' ? (
            <SettingsSidebar active={settingsPage} admin={admin} />
        ) : undefined
    const sidebarKey = view === 'agent' ? `agent:${routeAgent}` : view

    // A new screen settles into place: it dims and lifts a
    // little, it doesn't slide in from nowhere.
    const page = (
        <motion.div
            key={routeKey}
            className="flex min-h-0 min-w-0 flex-1 flex-col"
            initial={{ opacity: 0.7, y: 2 }}
            animate={{
                opacity: 1,
                y: 0,
                transition: { duration: motionDurations.base, ease: motionEases.arrive },
            }}
        >
            <Outlet />
        </motion.div>
    )

    if (mobile) {
        const drawerView = view === 'bare' && drawerAgent ? 'agent' : view
        return (
            <div
                className="flex h-dvh min-w-0 flex-col bg-panel text-fg"
                onClick={onClick}
            >
                {!ownsTopBar && (
                    <MobileTopBar
                        onMenu={() => setNavOpen(true)}
                        trailing={
                            <div className="flex items-center gap-4">
                                {pathname === '/' && can('all_agents:create') && (
                                    <IconButton
                                        icon="plus"
                                        aria-label="New agent"
                                        onClick={() => navigate('/agents/new')}
                                    />
                                )}
                                <NotificationsBell />
                            </div>
                        }
                    />
                )}
                <main className="flex min-h-0 min-w-0 flex-1 flex-col">{page}</main>
                <SlideOver open={navOpen} title="Menu" side="left" onClose={() => setNavOpen(false)}>
                    <div className="-m-20 flex h-[calc(100%+40px)] bg-shell">
                        <ShellRail pathname={pathname} view={view} routeAgent={routeAgent} />
                        {drawerView === 'agent' && (routeAgent ?? drawerAgent) ? (
                            <AgentSidebar agentId={(routeAgent ?? drawerAgent)!} />
                        ) : drawerView === 'settings' ? (
                            <SettingsSidebar active={settingsPage} admin={admin} />
                        ) : null}
                    </div>
                </SlideOver>
                <NotificationsPanel />
                <NotificationToasts />
                <ToastRegion />
            </div>
        )
    }

    return (
        <div className="contents" onClick={onClick}>
            <ShellFrame
                rail={<ShellRail pathname={pathname} view={view} routeAgent={routeAgent} />}
                sidebar={sidebar}
                sidebarKey={sidebarKey}
                aside={notificationsOpen ? <NotificationsAside /> : undefined}
            >
                {page}
            </ShellFrame>
            <NotificationToasts />
            <ToastRegion />
        </div>
    )
}
