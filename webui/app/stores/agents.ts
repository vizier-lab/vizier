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
    const ds = s.downloads.filter(
        (d) =>
            (typeof d.progress === 'number' && d.progress >= 0) ||
            (typeof d.completed === 'number' &&
                typeof d.total === 'number' &&
                d.total > 0)
    )
    if (!ds.length) return null
    return (
        ds.reduce((a, d) => {
            if (typeof d.progress === 'number')
                return a + Math.min(1, Math.max(0, d.progress))
            return a + (d.completed ?? 0) / (d.total ?? 1)
        }, 0) / ds.length
    )
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
            // Forget samples for agents that are no longer setting up, so a
            // later build starts measuring from scratch.
            const live = new Set(list.map((s) => s.agent_id))
            const samples: Record<string, SetupSample[]> = {}
            for (const [id, taken] of Object.entries(get().samples)) {
                if (live.has(id)) samples[id] = taken
            }
            for (const s of list) {
                const p = setupProgress(s)
                if (p == null) continue
                samples[s.agent_id] = [
                    ...(samples[s.agent_id] ?? []),
                    { at: now, progress: p },
                ].slice(-SAMPLES)
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
        // Always poll setup state while the tab is visible: a build can start
        // without any health saying "starting" yet, and the notification
        // panel's activity indicator must not wait for it (spec 029 US4).
        const setup = setInterval(() => {
            if (document.visibilityState === 'visible') void get().refreshSetups()
        }, SETUP_MS)
        void refreshSetups()
        return () => {
            clearInterval(health)
            clearInterval(setup)
        }
    },
}))
