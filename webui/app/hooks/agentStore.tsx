import { create } from 'zustand'
import { listAgents } from '../services/vizier'
import type { Agent } from '../interfaces/types'

interface AgentStore {
  agents: Agent[]
  loading: boolean
  loadAgents: () => Promise<void>
}

export const useAgentStore = create<AgentStore>()((set) => ({
  agents: [],
  loading: true,
  loadAgents: async () => {
    try {
      const response = await listAgents()
      set({ agents: response.data || [], loading: false })
    } catch (error) {
      console.error('Failed to load agents:', error)
      set({ loading: false })
    }
  },
}))
