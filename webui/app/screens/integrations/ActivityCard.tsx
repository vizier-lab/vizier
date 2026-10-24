import { ActivityItem, LoadMore, PanelHeader, Skeleton } from '@vizier/design'
import { Enter, stagger } from '../../lib/motion'
import { useCallback, useEffect, useState } from 'react'
import { integrationEvents, type IntegrationEvent } from '../../api/endpoints/integrations'
import { eventText } from '../../lib/integrations'
import { absoluteTime, relativeTime } from '../../lib/format'

type Props = {
    agentId: string
    /** Changes whenever the page changed something, to fetch the log again. */
    version?: number
}

/** The change record: who changed which integration, newest first (never the values). */
export function ActivityCard({ agentId, version = 0 }: Props) {
    const [events, setEvents] = useState<IntegrationEvent[]>([])
    const [next, setNext] = useState<string | null>()
    const [loading, setLoading] = useState(false)
    const [loaded, setLoaded] = useState(false)

    const load = useCallback(
        async (before?: string) => {
            setLoading(true)
            try {
                const page = await integrationEvents(agentId, before)
                setEvents((e) => (before ? [...e, ...page.items] : page.items))
                setNext(page.next ?? null)
            } catch {
                setNext(null)
            } finally {
                setLoading(false)
                setLoaded(true)
            }
        },
        [agentId]
    )

    useEffect(() => {
        void load()
    }, [load, version])

    return (
        <section aria-label="Activity" className="flex flex-col gap-12">
            <PanelHeader title="Activity" level={2} />
            {!loaded ? (
                <Skeleton variant="lines" lines={2} />
            ) : events.length === 0 ? (
                <p className="type-small text-faint">Nothing has changed yet. Every change to this agent’s integrations is listed here, with who made it.</p>
            ) : (
                <div className="flex flex-col">
                    {events.map((e, i) => (
                        <Enter key={e.id} delay={stagger(i, 0.03)}>
                            <ActivityItem actor={e.actor_name} time={<time dateTime={e.at} title={absoluteTime(e.at)}>{relativeTime(e.at)}</time>} text={eventText(e)} />
                        </Enter>
                    ))}
                    {next && <LoadMore label="Load older" loading={loading} onLoad={() => void load(next)} />}
                </div>
            )}
        </section>
    )
}
