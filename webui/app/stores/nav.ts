import { create } from 'zustand'

/** The phone layout's navigation drawer: opened by the top bar's ☰ on any screen. */
type NavStore = { open: boolean; setOpen: (open: boolean) => void }

export const useNav = create<NavStore>()((set) => ({
    open: false,
    setOpen: (open) => set({ open }),
}))
