import { ActivityItem, EmptyState, LoadMore, Sigil, Skeleton } from '@vizier/design'
import { useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router'
import { getFeed, type FeedEvent } from '../../api/endpoints/overview'
import { feedTime } from '../../lib/overview'
import { useAgents } from '../../stores/agents'

/** "Since you were away": what happened to your agents since you were last here. */
export function AwayFeed() {
    const navigate = useNavigate()
    const agents = useAgents((s) => s.agents)
    const [events, setEvents] = useState<FeedEvent[]>()
    const [next, setNext] = useState<string | null>()
    const [loading, setLoading] = useState(false)
    const [failed, setFailed] = useState(false)

    const load = useCallback(async (before?: string) => {
        setLoading(true)
        try {
            const page = await getFeed(before)
            // Start failures lead the first page only, so a later page never repeats one.
            setEvents((e) => (before ? [...(e ?? []), ...page.events] : page.events))
            setNext(page.next ?? null)
            setFailed(false)
        } catch {
            setFailed(true)
        } finally {
            setLoading(false)
        }
    }, [])
    useEffect(() => void load(), [load])

    const nameOf = (id: string) => agents.find((a) => a.agent_id === id)?.name ?? id

    return (
        <aside aria-label="Since you were away" className="flex flex-col gap-16">
            <h2 className="type-ui-strong text-fg">Since you were away</h2>
            {failed && !events ? (
                <EmptyState kind="error" title="Couldn’t load this" message="It will load again when you come back." />
            ) : !events ? (
                <div className="flex flex-col gap-12">
                    <Skeleton className="h-52" />
                    <Skeleton className="h-52" />
                    <Skeleton className="h-52" />
                </div>
            ) : events.length === 0 ? (
                <EmptyState kind="empty" icon="home" title="Nothing happened" message="No tasks ran, no dreams were had and nothing failed since you were last here." />
            ) : (
                <div className="flex flex-col">
                    {events.map((e) => (
                        <ActivityItem
                            key={e.id}
                            leading={<Sigil agentId={e.agent_id} size="sm" />}
                            actor={nameOf(e.agent_id)}
                            time={<time dateTime={e.at}>{feedTime(e.at)}</time>}
                            text={e.text}
                            failed={e.tone === 'failed'}
                            onSelect={() => navigate(e.href)}
                        />
                    ))}
                    {next && <LoadMore label="Load older" loading={loading} onLoad={() => void load(next)} />}
                </div>
            )}
        </aside>
    )
}
