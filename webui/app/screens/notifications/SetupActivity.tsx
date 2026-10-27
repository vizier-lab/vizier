import { AgentStartingCard, TextLink } from '@vizier/design'
import type { SetupState } from '../../api/endpoints/agents'
import { bytes } from '../../lib/format'
import { eta, setupProgress, useAgents } from '../../stores/agents'

type Props = {
    setup: SetupState
    /** Close the panel while the chat link navigates. */
    onOpen: () => void
}

function etaText(seconds: number | null): string | undefined {
    if (seconds == null) return undefined
    if (seconds < 60) return `about ${seconds} s`
    return `about ${Math.ceil(seconds / 60)} min`
}

function percentOf(d: SetupState['downloads'][number]): number | null {
    if (typeof d.progress === 'number')
        return Math.round(Math.min(1, Math.max(0, d.progress)) * 100)
    if (typeof d.completed === 'number' && typeof d.total === 'number' && d.total > 0)
        return Math.round((d.completed / d.total) * 100)
    return null
}

/**
 * One agent's live setup: what it is doing, how far along, and a line per
 * artifact. Live only — it has no read state and is never counted as unread.
 */
export function SetupActivity({ setup, onOpen }: Props) {
    const agent = useAgents((s) => s.agents.find((a) => a.agent_id === setup.agent_id))
    const samples = useAgents((s) => s.samples[setup.agent_id])
    const name = setup.name || agent?.name || setup.agent_id
    const pct = setupProgress(setup)
    const measured = setup.downloads.find(
        (d) => typeof d.completed === 'number' && typeof d.total === 'number' && (d.total ?? 0) > 0
    )
    const detail = measured
        ? `${bytes(measured.completed ?? 0)} of ${bytes(measured.total ?? 0)}`
        : undefined

    return (
        <div className="flex w-full flex-col gap-6">
            {/* The card caps its own width for toasts; in the panel it spans the column. */}
            <AgentStartingCard
                className="max-w-none!"
                name={name}
                step={setup.message || 'Building tools'}
                percent={pct == null ? undefined : Math.round(pct * 100)}
                detail={detail}
                eta={etaText(eta(samples))}
                downloads={
                    setup.downloads.length ? (
                        setup.downloads.map((d, i) => {
                            const percent = percentOf(d)
                            return (
                                <span
                                    key={`${d.kind ?? 'download'}-${i}`}
                                    className={`type-mono-xs ${d.error ? 'text-failed-text' : 'text-muted'}`}
                                >
                                    {d.kind ?? 'download'} · {d.label ?? d.name ?? ''}
                                    {percent != null && ` · ${percent}%`}
                                    {d.status && ` · ${d.status}`}
                                    {d.error && ` · ${d.error}`}
                                </span>
                            )
                        })
                    ) : undefined
                }
            />
            <TextLink href={`/${setup.agent_id}/chat`} onClick={onOpen}>
                Open chat →
            </TextLink>
        </div>
    )
}
