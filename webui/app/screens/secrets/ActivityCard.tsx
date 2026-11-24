import { ActivityItem, PanelHeader, Skeleton } from '@vizier/design'
import { useEffect, useState } from 'react'
import { listSecretEvents, type SecretEvent } from '../../api/endpoints/secrets'
import { absoluteTime, relativeTime } from '../../lib/format'
import { Enter, stagger } from '../../lib/motion'

type Props = {
    agentId: string
    /** Changes whenever the page changed something, to fetch the log again. */
    version?: number
}

const messageOf = (e: unknown, fallback: string) => (e instanceof Error && e.message ? e.message : fallback)

/** The secret store's change record: who created, replaced or deleted which name (never a value). */
export function ActivityCard({ agentId, version = 0 }: Props) {
    const [events, setEvents] = useState<SecretEvent[] | null>(null)
    const [error, setError] = useState<string>()

    useEffect(() => {
        let live = true
        setError(undefined)
        listSecretEvents(agentId, 20)
            .then((list) => live && setEvents(list.items))
            .catch((e) => live && setError(messageOf(e, 'The change log could not be loaded.')))
        return () => {
            live = false
        }
    }, [agentId, version])

    return (
        <section aria-label="Recent changes" className="flex flex-col gap-12">
            <PanelHeader title="Recent changes" level={2} />
            {error ? (
                <p className="type-small text-failed-text" role="alert">
                    {error}
                </p>
            ) : !events ? (
                <Skeleton variant="lines" lines={2} />
            ) : events.length === 0 ? (
                <p className="type-small text-faint">Nothing has changed yet. Every save and deletion is recorded here, with who made it.</p>
            ) : (
                <div className="flex flex-col">
                    {events.map((event, i) => (
                        <Enter key={event.id} delay={stagger(i, 0.03)}>
                            <ActivityItem
                                actor={event.actor_name}
                                time={
                                    <time dateTime={event.at} title={absoluteTime(event.at)}>
                                        {relativeTime(event.at)}
                                    </time>
                                }
                                text={
                                    <>
                                        {event.action} <span className="type-code">{event.name}</span>
                                    </>
                                }
                            />
                        </Enter>
                    ))}
                </div>
            )}
        </section>
    )
}
