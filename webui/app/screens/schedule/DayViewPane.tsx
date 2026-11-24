import { Button, DayTimeline, EmptyState, type DayBlock } from '@vizier/design'
import { useMemo, useState } from 'react'
import type { Task } from '../../api/endpoints/tasks'
import { useMobile } from '../../lib/useMobile'
import { addMonths, clock, dayKey, dayView, monthGrid } from '../../lib/schedule'
import { DayBlockCard } from './DayBlockCard'
import { MiniMonth } from './MiniMonth'
import { RepeatsCard } from './RepeatsCard'
import { momentKey } from './helpers'

const weekday = new Intl.DateTimeFormat('en-US', { weekday: 'long', day: 'numeric', month: 'long' })

type Props = { agentId: string; tasks: Task[]; day: Date; now: Date; onOpenDay: (key: string) => void }

/** One day: a small month to jump from, the tasks that repeat, and the day's moments by the hour. */
export function DayViewPane({ agentId, tasks, day, now, onOpenDay }: Props) {
    const mobile = useMobile()
    const view = useMemo(() => dayView(tasks, day, now), [tasks, day, now])
    const [folded, setFolded] = useState<string[] | null>(null)
    const [miniAnchor, setMiniAnchor] = useState<Date | null>(null)
    const anchor = miniAnchor ?? day
    const marked = useMemo(() => {
        const set = new Set<string>()
        for (const w of monthGrid(tasks, anchor, now)) for (const d of w) if (d.chips.length) set.add(d.key)
        return set
    }, [tasks, anchor, now])

    const today = dayKey(now) === dayKey(day)
    const blocks: DayBlock[] = view.blocks.map((m) => ({
        key: momentKey(m),
        minutes: m.at.getHours() * 60 + m.at.getMinutes(),
        node: <DayBlockCard agentId={agentId} moment={m} />,
    }))
    const hours = blocks.map((b) => Math.floor(b.minutes / 60))
    const fromHour = Math.min(7, ...hours, today ? now.getHours() : 7)
    const toHour = Math.max(21, ...hours, today ? now.getHours() : 21)
    const left = view.blocks.filter((m) => m.at > now).length
    const caption = `${new Intl.DateTimeFormat('en-US', { weekday: 'short' }).format(day)} ${day.getDate()} · ${
        today ? `${left} left today` : `${view.blocks.length} scheduled`
    } · times in your timezone`
    const foldedBlocks = folded ? view.blocks.filter((m) => folded.includes(momentKey(m))) : []

    return (
        <div className="grid gap-16 lg:grid-cols-[264px_minmax(0,1fr)]">
            <div className="flex min-h-0 flex-col gap-16">
                {!mobile && (
                    <MiniMonth
                        anchor={anchor}
                        selected={dayKey(day)}
                        today={dayKey(now)}
                        marked={marked}
                        onSelect={(k) => {
                            setMiniAnchor(null)
                            onOpenDay(k)
                        }}
                        onStepMonth={(by) => setMiniAnchor(addMonths(anchor, by))}
                    />
                )}
                {view.repeats.length > 0 && <RepeatsCard agentId={agentId} rows={view.repeats} />}
            </div>
            <div className="flex min-w-0 flex-col gap-12">
                {view.blocks.length === 0 && view.repeats.length === 0 ? (
                    <EmptyState kind="empty" icon="calendar" title="Nothing scheduled" message={`${weekday.format(day)} has no runs for these tasks.`} />
                ) : (
                    <div className="h-[min(674px,calc(100dvh-260px))] min-h-320 lg:h-[674px]">
                        <DayTimeline
                            label={weekday.format(day)}
                            caption={caption}
                            fromHour={fromHour}
                            toHour={toHour}
                            blocks={blocks}
                            now={today ? { minutes: now.getHours() * 60 + now.getMinutes(), label: clock(now) } : undefined}
                            onOpenMore={setFolded}
                        />
                    </div>
                )}
                {folded && (
                    <section aria-label="Also at this time" className="flex flex-col gap-8 rounded-xl border border-border bg-surface p-12">
                        <header className="flex items-center justify-between">
                            <h2 className="type-body-strong text-fg">Also at this time</h2>
                            <Button variant="ghost" size="sm" onClick={() => setFolded(null)}>
                                Hide
                            </Button>
                        </header>
                        <ul className="m-0 flex list-none flex-col gap-8 p-0">
                            {foldedBlocks.map((m) => (
                                <li key={momentKey(m)} className="h-44">
                                    <DayBlockCard agentId={agentId} moment={m} />
                                </li>
                            ))}
                        </ul>
                    </section>
                )}
            </div>
        </div>
    )
}

