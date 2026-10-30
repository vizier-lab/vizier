import { Icon, StatusDot, type StatusState } from '@vizier/design'
import { Link } from 'react-router'
import { useHere } from '../../lib/returnTo'
import { clock, type ChipState, type RepeatRow } from '../../lib/schedule'
import { taskHref } from './helpers'

const DOT: Partial<Record<ChipState, StatusState>> = {
    answered: 'answered',
    running: 'running',
    'no-response': 'no-response',
    interrupted: 'interrupted',
    upcoming: 'not-run',
}

/** Tasks that run more than once a day, so the timeline need not carry a block for every run. */
export function RepeatsCard({ agentId, rows }: { agentId: string; rows: RepeatRow[] }) {
    const here = useHere()
    return (
        <section aria-label="Repeats today" className="flex min-h-0 flex-col gap-12 overflow-hidden rounded-xl border border-border bg-surface p-16">
            <header className="flex flex-col gap-4">
                <div className="flex items-baseline justify-between">
                    <h2 className="type-heading text-fg">Repeats today</h2>
                    <span className="type-mono-xs text-faint">{rows.length}</span>
                </div>
                <p className="type-small text-muted">More than once a day. Soonest next run first.</p>
            </header>
            <ul className="m-0 flex min-h-0 list-none flex-col gap-8 overflow-y-auto p-0">
                {rows.map((r) => {
                    const done = r.moments.length - r.ahead
                    return (
                        <li key={r.task.slug} className="flex flex-col gap-8 rounded-md bg-raised p-12">
                            <Link to={taskHref(agentId, r.task)} state={here} className="flex min-w-0 items-center gap-8 type-body-medium text-fg hover:underline">
                                <Icon name="refresh" size={13} tone="muted" />
                                <span className="truncate">{r.task.title}</span>
                            </Link>
                            <span className="type-mono-xs text-faint">{r.cadence}</span>
                            {r.dense ? (
                                <>
                                    <div
                                        role="progressbar"
                                        aria-label={`${r.task.title}: ${done} of ${r.moments.length} moments passed today`}
                                        aria-valuemin={0}
                                        aria-valuemax={r.moments.length}
                                        aria-valuenow={done}
                                        className="h-6 overflow-hidden rounded-pill bg-surface"
                                    >
                                        <div className="h-full bg-ink" style={{ width: `${(done / r.moments.length) * 100}%` }} />
                                    </div>
                                    <span className="type-mono-xs text-muted">
                                        {r.ahead}
                                        {r.capped ? '+' : ''} left today{r.next ? ` · next ${clock(r.next)}` : ''}
                                    </span>
                                </>
                            ) : (
                                <ul className="m-0 flex list-none flex-wrap gap-4 p-0">
                                    {r.moments.map((m) => (
                                        <li
                                            key={m.at.toISOString()}
                                            className={`flex min-w-48 flex-col gap-4 rounded-sm bg-surface px-6 py-6 ${m.state === 'upcoming' ? 'border border-border-strong' : ''}`}
                                        >
                                            <span className="type-mono-xs text-soft">{clock(m.at)}</span>
                                            {DOT[m.state] ? (
                                                <StatusDot state={DOT[m.state]!} />
                                            ) : (
                                                <span aria-hidden className="flex size-12 items-center justify-center">
                                                    <span className="size-7 rounded-pill bg-unlit" />
                                                </span>
                                            )}
                                        </li>
                                    ))}
                                </ul>
                            )}
                        </li>
                    )
                })}
            </ul>
        </section>
    )
}
