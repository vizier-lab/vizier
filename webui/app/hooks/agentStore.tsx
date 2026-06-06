import { create } from 'zustand'
import { listAgents, getAgentsHealth } from '../services/vizier'
import type { Agent } from '../interfaces/types'

let healthInterval: ReturnType<typeof setInterval> | null = null

interface AgentStore {
  agents: Agent[]
  loading: boolean
  loadAgents: () => Promise<void>
  lastAgentId: string | null
  setLastAgentId: (id: string | null) => void
  agentHealth: Record<string, boolean>
  startHealthPolling: () => void
  stopHealthPolling: () => void
}

export const useAgentStore = create<AgentStore>()((set, get) => ({
  agents: [],
  loading: true,
  lastAgentId: null,
  agentHealth: {},
  setLastAgentId: (id) => set({ lastAgentId: id }),
  loadAgents: async () => {
    try {
      const response = await listAgents()
      set({ agents: response.data || [], loading: false })
    } catch (error) {
      console.error('Failed to load agents:', error)
      set({ loading: false })
    }
  },
  startHealthPolling: () => {
    if (healthInterval) return

    const poll = async () => {
      try {
        const statuses = await getAgentsHealth()
        const health: Record<string, boolean> = {}
        for (const s of statuses) {
          health[s.agent_id] = s.alive
        }
        set({ agentHealth: health })
      } catch {
        set({ agentHealth: {} })
      }
    }

    poll()
    healthInterval = setInterval(poll, 15_000)
  },
  stopHealthPolling: () => {
    if (healthInterval) {
      clearInterval(healthInterval)
      healthInterval = null
    }
  },
}))
