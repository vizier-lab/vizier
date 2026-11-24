import { create } from 'zustand'

/** Whether the app believes it can reach the engine. */
type ConnectivityStore = {
    offline: boolean
    setOffline: (offline: boolean) => void
}

export const useConnectivity = create<ConnectivityStore>()((set) => ({
    offline: false,
    setOffline: (offline) => set({ offline }),
}))
