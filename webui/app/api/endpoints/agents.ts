import { api } from '../client'

export type Agent = {
    agent_id: string
    name: string
    description?: string
    avatar_url?: string
    owner_username?: string
    owner_id?: string
    shared_to?: string[]
}

export type AgentRunState = 'starting' | 'running' | 'failed' | 'stopped'

export type AgentHealth = {
    agent_id: string
    alive: boolean
    state: AgentRunState
    last_error?: string
    failures: number
    next_retry_at?: string
    warnings?: string[]
}

export type SetupState = {
    agent_id: string
    phase: string
    message: string
    error?: string
    downloads: { name?: string; progress?: number; [k: string]: unknown }[]
}

export const listAgents = () => api.get<Agent[]>('/agents')
export const agentsHealth = () => api.get<AgentHealth[]>('/agents/health')
export const setupStates = () =>
    api.get<{ setups: SetupState[] }>('/utils/setup-state').then((r) => r?.setups ?? [])
