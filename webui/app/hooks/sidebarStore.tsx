import { create } from 'zustand'
import { persist } from 'zustand/middleware'

interface SidebarState {
  collapsed: boolean
  toggleSidebar: () => void
}

export const useSidebarStore = create<SidebarState>()(
  persist(
    (set) => ({
      collapsed: false,
      toggleSidebar: () => set(state => ({ collapsed: !state.collapsed })),
    }),
    {
      name: 'sidebar-storage',
      partialize: (state) => ({ collapsed: state.collapsed }),
    }
  )
)
