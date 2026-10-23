import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'

export interface Session {
  username: string
  setUsername: (username: string) => void
}

export const useSessionStore = create(
  persist(
    (set) => ({
      username: '',
      setUsername: (username: string) =>
        set((state: Session) => ({ ...state, username })),
    }),
    {
      name: 'vizer-session', // Unique name for the item in storage
      storage: createJSONStorage(() => localStorage), // Defaults to localStorage
    }
  )
)
