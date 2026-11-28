import { Badge, EmptyState, LoadMore, PanelHeader, RunRow, Skeleton, Status, Table, TableRow } from '@vizier/design'
import { useCallback, useEffect, useState } from 'react'
import { ApiError } from '../../api/client'
import { PAGE, triggerEvents, triggerRun, triggerRuns, type TriggerEvent, type TriggerRun, type TriggerRunDetail } from '../../api/endpoints/triggers'
import { absoluteTime, duration } from '../../lib/format'
import { replyText } from '../../lib/messages'
import { eventReason, outcome, payloadLine, RUN_STATES, runLabel } from '../../lib/triggers'
import { useMobile } from '../../lib/useMobile'
import { toast } from '../../stores/toasts'
import { Markdown } from '../shared/Markdown'

/** A newest-first list read page by page; a full page means there may be more. */
function usePages<T>(load: (before?: T) => Promise<T[]>, version: number) {
    const [items, setItems] = useState<T[] | null>(null)
    const [more, setMore] = useState(false)
    const [loading, setLoading] = useState(false)
    const read = useCallback(
        async (before?: T) => {
            setLoading(true)
            try {
                const page = await load(before)
                setItems((have) => (before ? [...(have ?? []), ...page] : page))
                setMore(page.length === PAGE)
            } catch (e) {
                // A trigger deleted under this list isn't a failed load: the page says it is gone.
                const gone = e instanceof ApiError && e.status === 404
                if (!gone) toast.error("Couldn't load what happened", e instanceof Error ? e.message : undefined)
                setItems((have) => have ?? [])
            } finally {
                setLoading(false)
            }
        },
        [load]
    )
    useEffect(() => void read(), [read, version])
    return { items, more, loading, next: () => items?.length && void read(items[items.length - 1]) }
}

const ran = (r: TriggerRun) => (r.finished_at ? duration(Date.parse(r.finished_at) - Date.parse(r.started_at)) : '–')

export function Events({ agentId, slug, version, onRun }: { agentId: string; slug: string; version: number; onRun: (id: number) => void }) {
    const mobile = useMobile()
    const load = useCallback((before?: TriggerEvent) => triggerEvents(agentId, slug, before), [agentId, slug])
    const { items, more, loading, next } = usePages(load, version)
    // A phone keeps what became of each event and why; its row still opens the run.
    const keep = <T,>(cells: T[]) => (mobile ? [cells[1], cells[2]] : cells)
    if (!items) return <Skeleton lines={5} />
    if (items.length === 0) return <EmptyState kind="empty" icon="zap" title="Nothing has happened yet" message="Every event is listed here with what became of it." />
    return (
        <div className="flex flex-col gap-12">
            <Table label="Events" columns={keep([{ label: 'Received', width: 150 }, { label: 'Outcome', width: mobile ? 130 : 170 }, { label: 'Why' }, { label: 'Run', width: 90 }])}>
                {items.map((e) => {
                    const o = outcome(e)
                    return (
                        <TableRow
                            key={e.id}
                            onSelect={e.run_id ? () => onRun(e.run_id!) : undefined}
                            cells={keep([
                                <span key="t" className="type-mono-xs text-muted">{absoluteTime(e.received_at)}</span>,
                                <Badge key="o" tone={o.tone}>{o.label}</Badge>,
                                <span key="w" className="flex min-w-0 flex-col gap-2">
                                    <span className="truncate type-small text-fg" title={eventReason(e)}>{eventReason(e)}</span>
                                    <span className="truncate type-mono-xs text-faint">{payloadLine(e.payload)}</span>
                                </span>,
                                <span key="r" className="type-mono-xs text-fg">{e.run_id ? `#${e.run_id}` : '–'}</span>,
                            ])}
                        />
                    )
                })}
            </Table>
            {more && <LoadMore label="Load older" loading={loading} onLoad={next} />}
            <p className="type-small text-faint">Payloads are deleted 3 days after they arrive. The record of what happened stays.</p>
        </div>
    )
}

export function Runs({ agentId, slug, version, selected, onSelect }: { agentId: string; slug: string; version: number; selected: number | null; onSelect: (id: number) => void }) {
    const load = useCallback((before?: TriggerRun) => triggerRuns(agentId, slug, before), [agentId, slug])
    const { items, more, loading, next } = usePages(load, version)
    const open = selected ?? items?.[0]?.id ?? null
    const [detail, setDetail] = useState<TriggerRunDetail | null | undefined>(undefined)
    useEffect(() => {
        if (open === null) return
        let live = true
        setDetail(undefined)
        triggerRun(agentId, slug, open)
            .then((d) => live && setDetail(d))
            .catch(() => live && setDetail(null))
        return () => {
            live = false
        }
    }, [agentId, slug, open, version])

    if (!items) return <Skeleton lines={5} />
    if (items.length === 0 && open === null) return <EmptyState kind="empty" icon="zap" title="Hasn’t run yet" message="Each run is listed here with what the agent answered." />
    const reply = detail ? [...detail.history].reverse().find((h) => h.content.Response && replyText(h.content.Response.content)) : undefined
    const answer = reply ? replyText(reply.content.Response!.content) : null
    return (
        <div className="flex flex-col gap-16 xl:flex-row xl:items-start">
            <div className="flex shrink-0 flex-col gap-2 xl:w-320">
                {items.map((r) => (
                    <RunRow key={r.id} time={absoluteTime(r.started_at)} state={RUN_STATES[r.state]} duration={ran(r)} selected={open === r.id} onSelect={() => onSelect(r.id)} />
                ))}
                {more && <LoadMore label="Load older" loading={loading} onLoad={next} />}
            </div>
            <section aria-label="Run" className="flex min-w-0 flex-1 flex-col gap-12">
                {detail === undefined ? (
                    <Skeleton lines={4} />
                ) : detail === null ? (
                    <EmptyState kind="error" title="This run isn’t there" message="It may belong to a trigger that was deleted." />
                ) : (
                    <>
                        <div className="flex flex-wrap items-center gap-12">
                            <PanelHeader title={`Run #${detail.run.id}`} level={2} className="flex-1" />
                            <Status state={RUN_STATES[detail.run.state]} label={runLabel(detail.run)} font="sans" detail={`${absoluteTime(detail.run.started_at)} · ${ran(detail.run)}`} />
                        </div>
                        {detail.run.quiet_note ? (
                            <p className="type-ui-para text-muted">{detail.run.quiet_note}</p>
                        ) : answer ? (
                            <Markdown size="document" entities={agentId}>{answer}</Markdown>
                        ) : (
                            <p className="type-small text-muted">{detail.run.state === 'running' ? 'It is working.' : 'This run answered nothing.'}</p>
                        )}
                        {detail.run.delivered_at && <p className="type-small text-faint">Delivered {absoluteTime(detail.run.delivered_at)}</p>}
                        <PanelHeader title="Events it was given" level={3} meta={`${detail.events.length}`} />
                        <ul className="flex flex-col gap-8">
                            {detail.events.map((e) => (
                                <li key={e.id} className="flex min-w-0 flex-col gap-2">
                                    <span className="type-small text-soft">
                                        {absoluteTime(e.received_at)}
                                        {(e.untriaged || e.reason) && ` · ${eventReason(e)}`}
                                    </span>
                                    <span className="type-mono-xs break-words text-faint">{payloadLine(e.payload)}</span>
                                </li>
                            ))}
                        </ul>
                    </>
                )}
            </section>
        </div>
    )
}


