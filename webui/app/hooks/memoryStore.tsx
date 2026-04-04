import { create } from 'zustand'
import { queryMemories } from '../services/vizier'
import type { Memory } from '../interfaces/types'

interface MemoryState {
  searchQuery: string
  searchResults: Memory[]
  isSearching: boolean
  setSearchQuery: (query: string) => void
  performSearch: (agentId: string, query: string) => Promise<void>
  clearSearch: () => void
}

export const useMemoryStore = create<MemoryState>((set) => ({
  searchQuery: '',
  searchResults: [],
  isSearching: false,
  setSearchQuery: (query) => set({ searchQuery: query }),
  performSearch: async (agentId, query) => {
    if (!query.trim()) {
      set({ searchResults: [], isSearching: false })
      return
    }
    set({ isSearching: true })
    try {
      const response = await queryMemories(agentId, query, 10)
      set({ searchResults: response.data || [], isSearching: false })
    } catch (error) {
      console.error('Memory search failed:', error)
      set({ searchResults: [], isSearching: false })
    }
  },
  clearSearch: () => set({ searchQuery: '', searchResults: [], isSearching: false }),
}))
