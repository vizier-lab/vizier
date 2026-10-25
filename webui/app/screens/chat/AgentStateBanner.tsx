import { Banner, Button, TextLink } from '@vizier/design'
import type { AgentHealth } from '../../api/endpoints/agents'
import type { ConnectionState } from '../../api/socket'
import { eta, setupProgress, type SetupSample } from '../../stores/agents'
import type { SetupState } from '../../api/endpoints/agents'
import { useNotifications } from '../../stores/notifications'
import { relativeTime } from '../../lib/format'

type Props = {
    agentName: string
    health?: AgentHealth
    setup?: SetupState
    samples?: SetupSample[]
    connection: ConnectionState
    /** The connection came back a moment ago. */
    reconnected?: boolean
    queued: number
    onRetryNow: () => void
    onReconnect: () => void
    configHref: string
}

function etaText(seconds: number | null): string | undefined {
    if (seconds == null) return undefined
    if (seconds < 60) return `about ${seconds} s`
    return `about ${Math.ceil(seconds / 60)} min`
}

/** Why the agent can't answer right now, or that the connection is down (FR-008). */
export function AgentStateBanner({ agentName, health, setup, samples, connection, reconnected, queued, onRetryNow, onReconnect, configHref }: Props) {
    const openNotifications = useNotifications((s) => s.openPanel)
    const lines = []

    // Setup is shown in full in the notification panel; here it is one line, so
    // the conversation keeps its room. The failure banner below stays: it is
    // where the person is blocked and can act.
    if (health?.state === 'starting') {
        const pct = setup ? setupProgress(setup) : null
        const remaining = etaText(eta(samples))
        lines.push(
            <p key="starting" className="flex flex-wrap items-center gap-6 type-small text-muted">
                <span aria-hidden className="size-6 shrink-0 rounded-pill bg-live animate-dot-wave motion-reduce:animate-none" />
                <span className="type-small text-fg">Starting {agentName}</span>
                <span aria-hidden>·</span>
                <span className="min-w-0 truncate">{setup?.message ?? 'Building tools'}</span>
                {pct != null && (
                    <>
                        <span aria-hidden>·</span>
                        <span>{Math.round(pct * 100)}%</span>
                    </>
                )}
                {remaining && (
                    <>
                        <span aria-hidden>·</span>
                        <span>{remaining}</span>
                    </>
                )}
                <span aria-hidden>·</span>
                <span>
                    {queued > 0
                        ? `${queued} message${queued === 1 ? '' : 's'} waiting`
                        : 'Messages wait in line'}
                </span>
                <TextLink onClick={openNotifications}>View in Notifications</TextLink>
            </p>
        )
    } else if (health?.state === 'failed') {
        lines.push(
            <Banner
                key="failed"
                tone="failed"
                title={`${agentName} couldn't start${health.failures > 1 ? ` · ${health.failures} failures` : ''}${health.next_retry_at ? ` · retry ${relativeTime(health.next_retry_at)}` : ''}`}
                message={health.last_error}
                actions={
                    <>
                        <Button size="sm" variant="secondary" onClick={onRetryNow}>
                            Retry now
                        </Button>
                        <Button size="sm" variant="ghost" onClick={() => location.assign(configHref)}>
                            Open config
                        </Button>
                    </>
                }
            />
        )
    }

    if (reconnected && connection === 'open') {
        lines.push(<Banner key="conn" tone="reconnected" title="Back online" />)
    } else if (connection === 'reconnecting') {
        lines.push(<Banner key="conn" tone="offline" title="Connection lost · reconnecting…" />)
    } else if (connection === 'offline') {
        lines.push(
            <Banner
                key="conn"
                tone="offline"
                title="You're offline"
                message="Messages can't be sent until the connection is back."
                actions={
                    <Button size="sm" variant="secondary" onClick={onReconnect}>
                        Retry
                    </Button>
                }
            />
        )
    }

    return lines.length ? <div className="flex flex-col gap-8 px-24 pt-12">{lines}</div> : null
}
