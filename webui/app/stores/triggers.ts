import { create } from 'zustand'
import { triggerSummary, type TriggerSummary } from '../api/endpoints/triggers'

type TriggerStore = {
    /** Each agent's triggers in numbers, for the sidebar badge. */
    summary: Record<string, TriggerSummary>
    /** Bumped when something happened to an agent's triggers, so an open page reads again. */
    version: Record<string, number>
    refresh: (agentId: string) => Promise<void>
    /** A trigger changed, or one reported: from this page's own action or from the inbox. */
    changed: (agentId: string) => void
}

export const useTriggers = create<TriggerStore>()((set, get) => ({
    summary: {},
    version: {},
    refresh: async (agentId) => {
        try {
            const summary = await triggerSummary(agentId)
            set((s) => ({ summary: { ...s.summary, [agentId]: summary } }))
        } catch {
            // No badge rather than a wrong one.
            set((s) => {
                const { [agentId]: _, ...rest } = s.summary
                return { summary: rest }
            })
        }
    },
    changed: (agentId) => {
        set((s) => ({ version: { ...s.version, [agentId]: (s.version[agentId] ?? 0) + 1 } }))
        // Only an agent whose badge is shown is asked for again.
        if (get().summary[agentId]) void get().refresh(agentId)
    },
}))
