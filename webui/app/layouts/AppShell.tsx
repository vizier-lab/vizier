import { Button, MobileTopBar, ShellFrame } from '@vizier/design'
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
import { NewAgentDialog } from '../screens/agents/NewAgentDialog'
import { SettingsSidebar } from '../screens/settings/SettingsSidebar'
import { AgentSidebar } from '../screens/shell/AgentSidebar'
import { WorkspaceColumn } from '../screens/workspaces/WorkspaceColumn'
import { useWorkspaces } from '../stores/workspaces'
import { viewOf } from '../screens/shell/pages'
import { ShellRail } from '../screens/shell/ShellRail'
import { MobileNavDrawer } from '../screens/shell/MobileNavDrawer'
import { AgentSwitcherSheet } from '../screens/shell/AgentSwitcherSheet'
import { detailsPath } from '../lib/agentDetails'
import { useSession } from '../stores/session'
import { getInstanceZone } from '../api/endpoints/timezone'
import { setInstanceZone } from '../lib/timezone'
import { ToastRegion } from './ToastRegion'

export default function AppShell() {
    const navigate = useNavigate()
    const { pathname } = useLocation()
    const params = useParams()
    const matches = useMatches()
    const { can } = useSession()
    const start = useAgents((s) => s.start)
    const agents = useAgents((s) => s.agents)
    const loadWorkspaces = useWorkspaces((s) => s.load)
    const workspaceList = useWorkspaces((s) => s.list)
    const mobile = useMobile()
    const installed = useInstalledIOS()

    // iOS scrolls the page up to keep a focused field above the keyboard, and leaves it there when
    // the keyboard goes away, so the whole shell sits a little high. Put the page back once focus
    // has left every field.
    useEffect(() => {
        if (!mobile) return
        let timer: number | undefined
        const settle = () => {
            window.clearTimeout(timer)
            timer = window.setTimeout(() => {
                const el = document.activeElement
                const typing = el instanceof HTMLElement && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName))
                if (!typing && (window.scrollY || window.visualViewport?.offsetTop)) window.scrollTo(0, 0)
            }, 150)
        }
        window.addEventListener('focusout', settle)
        window.visualViewport?.addEventListener('resize', settle)
        return () => {
            window.clearTimeout(timer)
            window.removeEventListener('focusout', settle)
            window.visualViewport?.removeEventListener('resize', settle)
        }
    }, [mobile])
    const setNavOpen = useNav((s) => s.setOpen)
    const sidebarCollapsed = useNav((s) => s.sidebarCollapsed)
    const toggleSidebar = useNav((s) => s.toggleSidebar)
    const resetNotifications = useNotifications((s) => s.reset)
    const setViewing = useNotifications((s) => s.setViewing)
    const setSwitcherOpen = useNav((s) => s.setSwitcherOpen)
    const lastAgent = useNav((s) => s.lastAgent)
    const setLastAgent = useNav((s) => s.setLastAgent)
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
    // Choosing a page closes the drawer and the agent switcher.
    useEffect(() => {
        setNavOpen(false)
        setSwitcherOpen(false)
    }, [pathname, setNavOpen, setSwitcherOpen])
    // A chat brings its own top bar, which has the topic and the agent's status.
    const ownsTopBar = /^\/[^/]+\/(?:projects\/[^/]+\/)?chat(\/|$)/.test(pathname)

    const routeAgent =
        params.agentId && params.agentId !== 'settings' && params.agentId !== 'agents'
            ? params.agentId
            : undefined
    const view = viewOf(pathname, routeAgent)
    // A phone keeps the last agent (the drawer's Chat, the mark on Home) while you are on Home or Settings.
    useEffect(() => {
        if (routeAgent) setLastAgent(routeAgent)
    }, [routeAgent, setLastAgent])
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
    const notices =
        unreachable ? (
            <div className="flex flex-col gap-8 px-24 pt-12">
                <OfflineBanner onRetry={retry} />
            </div>
        ) : null

    if (mobile) {
        const agentName = routeAgent ? (agents.find((a) => a.agent_id === routeAgent)?.name ?? routeAgent) : undefined
        const onDetails = !!routeAgent && pathname === detailsPath(routeAgent)
        const workspaceName = view === 'workspace' ? workspaceList.find((w) => w.id === params.workspaceId)?.name : undefined
        // Inside an agent the bar goes back up (a page to the agent's details, the details to its
        // chat); everywhere else it opens the menu.
        const bar =
            view === 'agent' && routeAgent
                ? onDetails
                    ? { onBack: () => navigate(`/${routeAgent}/chat`), backLabel: `Back to ${agentName}'s chat`, title: 'Agent' }
                    : { onBack: () => navigate(detailsPath(routeAgent)), backLabel: `Back to ${agentName}`, title: agentName }
                : {
                      onMenu: () => setNavOpen(true),
                      title: view === 'settings' ? 'Settings' : view === 'workspace' ? (workspaceName ?? 'Workspace') : 'Vizier',
                  }
        return (
            <div className="flex h-dvh min-w-0 flex-col bg-panel pb-safe text-fg" style={installed ? { height: '100lvh' } : undefined} onClick={onClick}>
                {!ownsTopBar && (
                    <MobileTopBar
                        {...bar}
                        trailing={
                            <div className="flex items-center">
                                {(onDetails || view === 'workspace') && (
                                    <Button variant="ghost" size="sm" trailingIcon="chevron-down" onClick={() => setSwitcherOpen(true)}>
                                        Switch
                                    </Button>
                                )}
                                <NotificationsBell />
                            </div>
                        }
                    />
                )}
                <main className="flex min-h-0 min-w-0 flex-1 flex-col">
                    {notices}
                    {page}
                </main>
                <MobileNavDrawer />
                <AgentSwitcherSheet current={routeAgent ?? lastAgent} />
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
                {notices}
                {page}
            </ShellFrame>
            <NotificationsPopup />
            <NotificationToasts />
            <ToastRegion />
            <NewAgentDialog />
        </div>
    )
}

/**
 * The installed app on iOS draws the page over the whole screen, but once the keyboard has been
 * up `dvh` comes back short (812 of 874 on an iPhone 16 Pro) and stays there, lifting the composer.
 * `lvh` does not move, and in the installed app there is no browser toolbar for it to count. The
 * shell stays in the flow: a fixed one is cut at the shorter viewport and leaves a black band.
 */
function useInstalledIOS(): boolean {
    const [installed, setInstalled] = useState(false)
    useEffect(() => {
        setInstalled((navigator as Navigator & { standalone?: boolean }).standalone === true)
    }, [])
    return installed
}
