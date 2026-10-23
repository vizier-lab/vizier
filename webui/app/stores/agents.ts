import { create } from 'zustand'
import {
    agentsHealth,
    listAgents,
    setupStates,
    type Agent,
    type AgentHealth,
    type SetupState,
} from '../api/endpoints/agents'

const HEALTH_MS = 10_000
const SETUP_MS = 1_000
const SAMPLES = 5

export type SetupSample = { at: number; progress: number }

type AgentStore = {
    agents: Agent[]
    health: Record<string, AgentHealth>
    setups: Record<string, SetupState>
    /** The last few progress samples per starting agent, for the ETA (research R15). */
    samples: Record<string, SetupSample[]>
    loaded: boolean
    refresh: () => Promise<void>
    refreshHealth: () => Promise<void>
    refreshSetups: () => Promise<void>
    /** Polls while the tab is visible; returns a stop function. */
    start: () => () => void
}

/** 0..1 from a setup's downloads, or null when it reports none. */
export function setupProgress(s: SetupState): number | null {
    const ds = s.downloads.filter((d) => typeof d.progress === 'number')
    if (!ds.length) return null
    return ds.reduce((a, d) => a + (d.progress as number), 0) / ds.length
}

/** Seconds left, extrapolated from the recent samples; null until there are two that moved. */
export function eta(samples: SetupSample[] | undefined): number | null {
    if (!samples || samples.length < 2) return null
    const a = samples[0]
    const b = samples[samples.length - 1]
    const rate = (b.progress - a.progress) / ((b.at - a.at) / 1000)
    if (!(rate > 0)) return null
    return Math.ceil((1 - b.progress) / rate)
}

export const useAgents = create<AgentStore>()((set, get) => ({
    agents: [],
    health: {},
    setups: {},
    samples: {},
    loaded: false,

    refresh: async () => {
        try {
            const agents = await listAgents()
            set({ agents: agents ?? [], loaded: true })
        } catch {
            set({ loaded: true })
        }
    },

    refreshHealth: async () => {
        try {
            const list = await agentsHealth()
            set({ health: Object.fromEntries((list ?? []).map((h) => [h.agent_id, h])) })
        } catch {
            /* keep the last known health */
        }
    },

    refreshSetups: async () => {
        try {
            const list = await setupStates()
            const now = Date.now()
            const samples = { ...get().samples }
            for (const s of list) {
                const p = setupProgress(s)
                if (p == null) continue
                samples[s.agent_id] = [...(samples[s.agent_id] ?? []), { at: now, progress: p }].slice(-SAMPLES)
            }
            set({ setups: Object.fromEntries(list.map((s) => [s.agent_id, s])), samples })
        } catch {
            /* ignore */
        }
    },

    start: () => {
        const { refresh, refreshHealth, refreshSetups } = get()
        void refresh()
        void refreshHealth()
        const health = setInterval(() => {
            if (document.visibilityState === 'visible') void get().refreshHealth()
        }, HEALTH_MS)
        const setup = setInterval(() => {
            if (document.visibilityState !== 'visible') return
            const starting = Object.values(get().health).some((h) => h.state === 'starting')
            if (starting || Object.keys(get().setups).length) void get().refreshSetups()
        }, SETUP_MS)
        void refreshSetups()
        return () => {
            clearInterval(health)
            clearInterval(setup)
        }
    },
}))
