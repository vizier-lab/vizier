import { IconButton, MobileTopBar, ShellFrame, SlideOver } from '@vizier/design'
import { useEffect, useMemo, useState, type MouseEvent } from 'react'
import { Outlet, useLocation, useMatches, useNavigate, useParams } from 'react-router'
import { motion } from 'motion/react'
import { motionDurations, motionEases } from '@vizier/design'
import { useMobile } from '../lib/useMobile'
import { isUnreachable } from '../lib/pwa/connectivity'
import { useNav } from '../stores/nav'
import { useAgents } from '../stores/agents'
import { useConnection } from '../stores/connection'
import { useConnectivity } from '../stores/connectivity'
import { useNotifications } from '../stores/notifications'
import { NotificationsPopup } from '../screens/notifications/NotificationsPopup'
import { NotificationsBell } from '../screens/shared/NotificationsBell'
import { NotificationToasts } from '../screens/shared/NotificationToasts'
import { OfflineBanner } from '../screens/shared/OfflineBanner'
import { NewAgentDialog, withNewAgent } from '../screens/agents/NewAgentDialog'
import { SettingsSidebar } from '../screens/settings/SettingsSidebar'
import { AgentSidebar } from '../screens/shell/AgentSidebar'
import { WorkspaceColumn } from '../screens/workspaces/WorkspaceColumn'
import { useWorkspaces } from '../stores/workspaces'
import { viewOf } from '../screens/shell/pages'
import { ShellRail } from '../screens/shell/ShellRail'
import { useSession } from '../stores/session'
import { getInstanceZone } from '../api/endpoints/timezone'
import { setInstanceZone } from '../lib/timezone'
import { ToastRegion } from './ToastRegion'

export default function AppShell() {
    const navigate = useNavigate()
    const { pathname, search } = useLocation()
    const params = useParams()
    const matches = useMatches()
    const { can } = useSession()
    const start = useAgents((s) => s.start)
    const loadWorkspaces = useWorkspaces((s) => s.load)
    const mobile = useMobile()
    const navOpen = useNav((s) => s.open)
    const setNavOpen = useNav((s) => s.setOpen)
    const sidebarCollapsed = useNav((s) => s.sidebarCollapsed)
    const toggleSidebar = useNav((s) => s.toggleSidebar)
    const resetNotifications = useNotifications((s) => s.reset)
    const setViewing = useNotifications((s) => s.setViewing)
    const [drawerAgent, setDrawerAgent] = useState<string>()
    // Times are drawn in the instance's zone, so the pages wait for it. If it can't be read they
    // carry on in the browser's zone rather than show nothing.
    const [zoneKnown, setZoneKnown] = useState(false)
    useEffect(() => {
        getInstanceZone()
            .then(setInstanceZone)
            .catch((e) => console.warn("Couldn't read the instance's timezone; using the browser's", e))
            .finally(() => setZoneKnown(true))
    }, [])

    useEffect(() => start(), [start])
    useEffect(() => void loadWorkspaces(), [loadWorkspaces])
    // Whether the app can reach the engine: what the browser says, or a topic that lost its
    // connection. The banner says what is unavailable rather than pretending the app is fine.
    const offline = useConnectivity((s) => s.offline)
    const topicStates = useConnection((s) => s.state)
    const unreachable = useMemo(() => isUnreachable(offline, topicStates), [offline, topicStates])
    useEffect(() => {
        const update = () => useConnectivity.getState().setOffline(!navigator.onLine)
        update()
        window.addEventListener('online', update)
        window.addEventListener('offline', update)
        return () => {
            window.removeEventListener('online', update)
            window.removeEventListener('offline', update)
        }
    }, [])
    // Trying again re-reads the browser's answer and asks every lost socket to reconnect.
    const retry = () => {
        useConnectivity.getState().setOffline(!navigator.onLine)
        for (const [key, state] of Object.entries(topicStates)) {
            if (state !== 'offline') continue
            const cut = key.lastIndexOf('/')
            useConnection.getState().retry(key.slice(0, cut), key.slice(cut + 1))
        }
    }
    // A fresh session starts from zero, then the badge is kept current by the inbox stream.
    useEffect(() => {
        resetNotifications()
        return useNotifications.getState().start()
    }, [resetNotifications])
    // A reply to the conversation on screen is read on arrival, not toasted.
    useEffect(() => setViewing(pathname), [pathname, setViewing])
    // Choosing a page closes the drawer.
    useEffect(() => setNavOpen(false), [pathname, setNavOpen])
    // A chat brings its own top bar, which has the topic and the agent's status.
    const ownsTopBar = /^\/[^/]+\/(?:projects\/[^/]+\/)?chat(\/|$)/.test(pathname)

    const routeAgent =
        params.agentId && params.agentId !== 'settings' && params.agentId !== 'agents'
            ? params.agentId
            : undefined
    const view = viewOf(pathname, routeAgent)
    // The drawer on a phone keeps showing the last agent while you are on Home or Settings.
    useEffect(() => {
        if (routeAgent) setDrawerAgent(routeAgent)
    }, [routeAgent])
    // Ctrl B (or ⌘B) shuts the agent column. A rich text editor keeps the key for bold.
    useEffect(() => {
        if (mobile || view !== 'agent') return
        const on = (e: KeyboardEvent) => {
            if (!(e.ctrlKey || e.metaKey) || e.shiftKey || e.altKey || e.key.toLowerCase() !== 'b') return
            if (e.defaultPrevented || (e.target as HTMLElement | null)?.isContentEditable) return
            e.preventDefault()
            toggleSidebar()
        }
        window.addEventListener('keydown', on)
        return () => window.removeEventListener('keydown', on)
    }, [mobile, view, toggleSidebar])

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
        ) : view === 'workspace' ? (
            <WorkspaceColumn />
        ) : undefined
    const sidebarKey = view === 'agent' ? `agent:${routeAgent}` : view === 'workspace' ? `workspace:${params.workspaceId}` : view

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
            {zoneKnown && <Outlet />}
        </motion.div>
    )

    // Above the page, in the same place the chat draws its own connection banner.
    const banner = unreachable ? (
        <div className="px-24 pt-12">
            <OfflineBanner onRetry={retry} />
        </div>
    ) : null

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
                                        onClick={() => navigate(withNewAgent(`${pathname}${search}`))}
                                    />
                                )}
                                <NotificationsBell />
                            </div>
                        }
                    />
                )}
                <main className="flex min-h-0 min-w-0 flex-1 flex-col">
                    {banner}
                    {page}
                </main>
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
                <NotificationsPopup />
                <NotificationToasts />
                <ToastRegion />
                <NewAgentDialog />
            </div>
        )
    }

    return (
        <div className="contents" onClick={onClick}>
            <ShellFrame
                rail={<ShellRail pathname={pathname} view={view} routeAgent={routeAgent} />}
                sidebar={sidebar}
                sidebarKey={sidebarKey}
                collapsed={view === 'agent' && sidebarCollapsed}
            >
                {banner}
                {page}
            </ShellFrame>
            <NotificationsPopup />
            <NotificationToasts />
            <ToastRegion />
            <NewAgentDialog />
        </div>
    )
}
