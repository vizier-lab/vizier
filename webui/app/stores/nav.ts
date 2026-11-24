import { create } from 'zustand'
import { read, write } from '../lib/remember'

const COLLAPSED_KEY = 'vizier.sidebar.collapsed'
const LAST_AGENT_KEY = 'vizier.nav.lastAgent'

type NavStore = {
    /** The phone layout's navigation drawer: opened by the top bar's ☰ on any screen. */
    open: boolean
    setOpen: (open: boolean) => void
    /** The phone's agent switcher sheet, opened from the agent's name in the top bar. */
    switcherOpen: boolean
    setSwitcherOpen: (open: boolean) => void
    /**
     * The agent the person was last on. The phone's top bar, Chat in the drawer and the mark on
     * Home keep showing it while the person is on Home or Settings.
     */
    lastAgent?: string
    setLastAgent: (agentId: string) => void
    /** The desktop agent column is shut, leaving only the rail. Remembered per browser. */
    sidebarCollapsed: boolean
    toggleSidebar: () => void
}

export const useNav = create<NavStore>()((set, get) => ({
    open: false,
    setOpen: (open) => set({ open }),
    switcherOpen: false,
    setSwitcherOpen: (switcherOpen) => set({ switcherOpen }),
    lastAgent: read<string | undefined>(LAST_AGENT_KEY, undefined),
    setLastAgent: (lastAgent) => {
        if (get().lastAgent === lastAgent) return
        write(LAST_AGENT_KEY, lastAgent)
        set({ lastAgent })
    },
    sidebarCollapsed: read(COLLAPSED_KEY, false),
    toggleSidebar: () => {
        const sidebarCollapsed = !get().sidebarCollapsed
        write(COLLAPSED_KEY, sidebarCollapsed)
        set({ sidebarCollapsed })
    },
}))
