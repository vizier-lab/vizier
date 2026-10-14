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
  /** Why an agent is down, for agents whose supervisor reports a failure. */
  agentFailure: Record<string, string>
  startHealthPolling: () => void
  stopHealthPolling: () => void
}

export const useAgentStore = create<AgentStore>()((set, get) => ({
  agents: [],
  loading: true,
  lastAgentId: null,
  agentHealth: {},
  agentFailure: {},
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
        const failure: Record<string, string> = {}
        for (const s of statuses) {
          health[s.agent_id] = s.alive
          if (s.state === 'failed' && s.last_error) {
            failure[s.agent_id] = s.last_error
          }
        }
        set({ agentHealth: health, agentFailure: failure })
      } catch {
        set({ agentHealth: {}, agentFailure: {} })
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
