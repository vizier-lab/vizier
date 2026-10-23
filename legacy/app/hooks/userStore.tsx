import { create } from 'zustand'
import { getCurrentUser } from '../services/vizier'
import type { CurrentUser } from '../interfaces/types'

interface UserStore {
  user: CurrentUser | null
  loading: boolean
  loadUser: () => Promise<void>
  clearUser: () => void
}

export const useUserStore = create<UserStore>()((set) => ({
  user: null,
  loading: true,
  loadUser: async () => {
    try {
      const response = await getCurrentUser()
      set({ user: response.data || null, loading: false })
    } catch (error) {
      console.error('Failed to load current user:', error)
      set({ user: null, loading: false })
    }
  },
  clearUser: () => set({ user: null, loading: false }),
}))
