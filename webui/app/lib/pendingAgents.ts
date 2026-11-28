// Agents still being created, for the rail's busy tiles.
import type { SetupState } from '../api/endpoints/agents'

export type PendingAgent = { agentId: string; name: string }

const building = (s: SetupState) => s.phase === 'starting' || s.phase === 'downloading'

/**
 * The agents that are being built but are not stored yet. `creating` is what this tab asked for
 * (lost on a reload); `setups` is what the server reports, which outlives the page: an agent is
 * stored only once its build succeeds, so a build for an id the list does not have is a creation.
 * One entry per agent, this tab's own first.
 */
export function pendingAgents(
    creating: PendingAgent[],
    setups: Record<string, SetupState>,
    known: (agentId: string) => boolean
): PendingAgent[] {
    const out: PendingAgent[] = []
    const seen = new Set<string>()
    const add = (p: PendingAgent) => {
        if (seen.has(p.agentId) || known(p.agentId)) return
        seen.add(p.agentId)
        out.push(p)
    }
    creating.forEach(add)
    Object.values(setups)
        .filter(building)
        .sort((a, b) => a.agent_id.localeCompare(b.agent_id))
        .forEach((s) => add({ agentId: s.agent_id, name: s.name || s.agent_id }))
    return out
}
