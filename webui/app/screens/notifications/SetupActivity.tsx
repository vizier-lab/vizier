import { SetupItem } from '@vizier/design'
import type { SetupState } from '../../api/endpoints/agents'
import { bytes } from '../../lib/format'
import { eta, setupProgress, useAgents } from '../../stores/agents'

type Props = {
    setup: SetupState
}

function etaText(seconds: number | null): string | undefined {
    if (seconds == null) return undefined
    if (seconds < 60) return `about ${seconds} s`
    return `about ${Math.ceil(seconds / 60)} min`
}

/**
 * One agent's live setup, in the same card shape as a notification. One row per agent: what it
 * is doing, how far along, the size and the time left. Live only: it has no read state and is
 * never counted as unread.
 */
export function SetupActivity({ setup }: Props) {
    const agent = useAgents((s) => s.agents.find((a) => a.agent_id === setup.agent_id))
    const samples = useAgents((s) => s.samples[setup.agent_id])
    const name = setup.name || agent?.name || setup.agent_id
    const pct = setupProgress(setup)
    const measured = setup.downloads.find(
        (d) => typeof d.completed === 'number' && typeof d.total === 'number' && (d.total ?? 0) > 0
    )
    const label = measured?.label ?? measured?.name
    const size = measured
        ? `${bytes(measured.completed ?? 0)} of ${bytes(measured.total ?? 0)}`
        : undefined
    const detail = [label, size].filter(Boolean).join(' · ') || undefined

    return (
        <SetupItem
            agentId={setup.agent_id}
            agentName={name}
            step={setup.message || 'Building tools'}
            percent={pct == null ? undefined : Math.round(pct * 100)}
            detail={detail}
            eta={etaText(eta(samples))}
        />
    )
}
