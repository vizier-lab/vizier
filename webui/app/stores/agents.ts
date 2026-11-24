import { create } from 'zustand'
import { agentsHealth, listAgents, type Agent, type AgentHealth, type SetupState } from '../api/endpoints/agents'
import { TERMINAL_MS, dropTerminal, isTerminal, onHelloSetups, onSetup, onSetupCleared } from '../lib/inboxFrames'
import { eta, setupProgress, type SetupSample } from '../lib/setupProgress'

// The setup helpers live in lib/ so frame handling can use them; importers keep this path.
export { eta, setupProgress, type SetupSample }

const HEALTH_MS = 10_000

/** The timer that drops each agent's finished setup, so a newer frame can cancel it. */
const terminalTimers = new Map<string, ReturnType<typeof setTimeout>>()

/** An agent whose creation is still running on the server: shown in the rail, busy, until it lands. */
export type CreatingAgent = { agentId: string; name: string }

type AgentStore = {
    agents: Agent[]
    /** Creations in flight, newest last. Cleared one at a time as each finishes. */
    creating: CreatingAgent[]
    health: Record<string, AgentHealth>
    setups: Record<string, SetupState>
    /** The last few progress samples per starting agent, for the ETA (research R15). */
    samples: Record<string, SetupSample[]>
    loaded: boolean
    refresh: () => Promise<void>
    refreshHealth: () => Promise<void>
    /** One agent's setup changed (an inbox `setup` frame). */
    applySetup: (state: SetupState) => void
    /** One agent's setup is gone (`setup_cleared`). */
    clearSetup: (agentId: string) => void
    /** A snapshot replaces every setup (`hello`). */
    replaceSetups: (list: SetupState[]) => void
    /** Forget every setup, e.g. on sign-out. */
    resetSetups: () => void
    /** Show an agent that is being created, before the server has it. */
    startCreating: (agent: CreatingAgent) => void
    /** The creation finished, one way or another: drop the busy tile. */
    finishCreating: (agentId: string) => void
    /** Polls while the tab is visible; returns a stop function. */
    start: () => () => void
}

export const useAgents = create<AgentStore>()((set, get) => ({
    agents: [],
    creating: [],
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

    applySetup: (state) => {
        set(onSetup(get(), state, Date.now()))
        watchTerminal(state)
    },

    clearSetup: (agentId) => {
        cancelTerminal(agentId)
        set(onSetupCleared(get(), agentId))
    },

    replaceSetups: (list) => {
        terminalTimers.forEach((timer) => clearTimeout(timer))
        terminalTimers.clear()
        set(onHelloSetups(get(), list, Date.now()))
        list.forEach(watchTerminal)
    },

    resetSetups: () => {
        terminalTimers.forEach((timer) => clearTimeout(timer))
        terminalTimers.clear()
        set({ setups: {}, samples: {} })
    },

    startCreating: (agent) =>
        set((s) => ({
            creating: [...s.creating.filter((c) => c.agentId !== agent.agentId), agent],
        })),

    finishCreating: (agentId) =>
        set((s) => ({ creating: s.creating.filter((c) => c.agentId !== agentId) })),

    start: () => {
        const { refresh, refreshHealth } = get()
        void refresh()
        void refreshHealth()
        const health = setInterval(() => {
            if (document.visibilityState === 'visible') void get().refreshHealth()
        }, HEALTH_MS)
        // Setup state is not polled: it arrives on the inbox stream (`stores/notifications.ts`
        // opens it), so a build that starts shows at once without a timer.
        return () => clearInterval(health)
    },
}))

function cancelTerminal(agentId: string) {
    const timer = terminalTimers.get(agentId)
    if (timer) clearTimeout(timer)
    terminalTimers.delete(agentId)
}

/**
 * A finished setup is dropped after a few seconds: the server prunes it only when it is next
 * read, so a client cannot wait to be told. A newer frame for the agent cancels the timer.
 */
function watchTerminal(state: SetupState) {
    cancelTerminal(state.agent_id)
    if (!isTerminal(state)) return
    terminalTimers.set(
        state.agent_id,
        setTimeout(() => {
            terminalTimers.delete(state.agent_id)
            const next = dropTerminal(useAgents.getState(), state)
            if (next) useAgents.setState(next)
        }, TERMINAL_MS)
    )
}
