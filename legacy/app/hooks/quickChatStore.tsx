import { create } from 'zustand'

interface QuickChatStore {
  pendingMessage: string | null
  setPendingMessage: (msg: string | null) => void
  consumePendingMessage: () => string | null
}

export const useQuickChatStore = create<QuickChatStore>()((set, get) => ({
  pendingMessage: null,
  setPendingMessage: (msg) => set({ pendingMessage: msg }),
  consumePendingMessage: () => {
    const msg = get().pendingMessage
    set({ pendingMessage: null })
    return msg
  },
}))
