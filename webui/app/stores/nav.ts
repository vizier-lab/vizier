import { create } from 'zustand'
import { read, write } from '../lib/remember'

const COLLAPSED_KEY = 'vizier.sidebar.collapsed'

type NavStore = {
    /** The phone layout's navigation drawer: opened by the top bar's ☰ on any screen. */
    open: boolean
    setOpen: (open: boolean) => void
    /** The desktop agent column is shut, leaving only the rail. Remembered per browser. */
    sidebarCollapsed: boolean
    toggleSidebar: () => void
}

export const useNav = create<NavStore>()((set, get) => ({
    open: false,
    setOpen: (open) => set({ open }),
    sidebarCollapsed: read(COLLAPSED_KEY, false),
    toggleSidebar: () => {
        const sidebarCollapsed = !get().sidebarCollapsed
        write(COLLAPSED_KEY, sidebarCollapsed)
        set({ sidebarCollapsed })
    },
}))
