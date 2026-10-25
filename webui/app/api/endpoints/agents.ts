import type { AgentState } from '@vizier/design'
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
    /** A turn is running now. */
    live?: boolean
    state: AgentRunState
    last_error?: string
    failures: number
    next_retry_at?: string
    warnings?: string[]
    model?: { provider: string; model: string; context_window?: number | null; max_output?: number | null }
}

export type SetupDownload = {
    kind?: string
    label?: string
    /** Bytes done; `total` 0 means the size is unknown. */
    completed?: number
    total?: number
    status?: string
    error?: string
    /** Legacy/derived 0..1, when a source reports it. */
    progress?: number
    name?: string
    [k: string]: unknown
}

export type SetupState = {
    agent_id: string
    /** The agent's display name; a create build may not be in the agent store yet. */
    name?: string
    phase: string
    message: string
    error?: string
    downloads: SetupDownload[]
}

export const listAgents = () => api.get<Agent[]>('/agents')
export const agentsHealth = () => api.get<AgentHealth[]>('/agents/health')
export const setupStates = () =>
    api.get<{ setups: SetupState[] }>('/utils/setup-state').then((r) => r?.setups ?? [])

export type ShellConfig =
    | { environment: 'local'; path: string; env?: Record<string, string> | null }
    | {
          environment: 'docker'
          image: { source: 'pull'; name: string } | { source: 'dockerfile'; path: string; name: string }
          container_name: string
          env?: Record<string, string> | null
      }

export type ReadImageSettings = { provider?: string | null; model?: string | null }

/** `GET /agents/{id}`. */
export type AgentDetail = {
    agent_id: string
    name: string
    description?: string | null
    provider: string
    model: string
    system_prompt?: string | null
    thinking_depth: number
    checkpoint_threshold: number
    max_tokens?: number | null
    context_window?: number | null
    shell?: ShellConfig | null
    fetch: boolean
    http_client: boolean
    prompt_timeout: string
    dream_enabled: boolean
    dream_schedule?: string | null
    dream_provider?: string | null
    dream_model?: string | null
    tools_timeout: string
    avatar_url?: string | null
    silent_read_initiative_chance: number
    tts: boolean
    tts_settings?: Record<string, unknown> | null
    stt: boolean
    stt_settings?: Record<string, unknown> | null
    read_image: boolean
    read_image_settings?: ReadImageSettings | null
    image_gen: boolean
    image_gen_settings?: Record<string, unknown> | null
    embedding?: { provider: string; model: string } | null
    indexer?: { kind: string } | null
}

/** `POST`/`PUT /agents`. */
export type AgentInput = {
    agent_id: string
    name: string
    description?: string | null
    provider: string
    model: string
    system_prompt?: string | null
    thinking_depth?: number
    checkpoint_threshold?: number
    max_tokens?: number | null
    context_window?: number | null
    prompt_timeout?: string
    dream_enabled?: boolean
    dream_schedule?: string | null
    dream_provider?: string | null
    dream_model?: string | null
    avatar_url?: string | null
    silent_read_initiative_chance?: number
    embedding?: { provider: string; model: string } | null
    indexer?: { kind: string } | null
    tools?: {
        shell?: ShellConfig | null
        fetch?: boolean
        http_client?: boolean
        timeout?: string
        tts?: boolean
        tts_settings?: Record<string, unknown> | null
        stt?: boolean
        stt_settings?: Record<string, unknown> | null
        read_image?: boolean
        read_image_settings?: ReadImageSettings | null
        image_gen?: boolean
        image_gen_settings?: Record<string, unknown> | null
    }
}

/** A saved agent, with anything about its model worth knowing. */
export type AgentSaved = Agent & { warnings: { code?: string; message?: string }[] }

export const getAgent = (id: string) => api.get<AgentDetail>(`/agents/${encodeURIComponent(id)}`)
export const createAgent = (input: AgentInput) => api.post<AgentSaved>('/agents', input)
export const updateAgent = (input: AgentInput) => api.put<AgentSaved>(`/agents/${encodeURIComponent(input.agent_id)}`, input)
export const deleteAgent = (id: string) => api.delete<string>(`/agents/${encodeURIComponent(id)}`)
export const getSharing = (id: string) => api.get<{ shared_to: string[] }>(`/agents/${encodeURIComponent(id)}/sharing`)
export const updateSharing = (id: string, change: { add?: string[]; remove?: string[] }) =>
    api.patch<{ shared_to: string[] }>(`/agents/${encodeURIComponent(id)}/sharing`, change)

/** The dot an agent shows: up is live, a turn in progress is running; failures and start-up states beat both. */
export function agentState(h?: AgentHealth): AgentState {
    if (!h) return 'live' 
    if (h.state === 'failed') return h.next_retry_at ? 'retrying' : 'failed'
    if (h.state === 'starting') return 'starting'
    if (h.state === 'stopped') return 'stopped'
    return h.live ? 'running' : 'live'
}
