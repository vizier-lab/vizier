import { Button, Floating, IconButton, MonthGrid, ScheduleChip, useDismiss, type MonthGridDay } from '@vizier/design'
import { useMemo, useRef, useState } from 'react'
import { useNavigate } from 'react-router'
import type { Task } from '../../api/endpoints/tasks'
import { dayKey, monthGrid, type DayChip } from '../../lib/schedule'
import { useHere } from '../../lib/returnTo'
import { chipRepeat, chipTime, taskHref } from './helpers'

const WEEKDAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']
/** Chips a cell shows before the rest fold into a count. */
const FIT = 3

const dayName = new Intl.DateTimeFormat('en-US', { weekday: 'long', day: 'numeric', month: 'long' })
const firstOfMonth = new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric' })

type Props = { agentId: string; tasks: Task[]; anchor: Date; now: Date; onOpenDay: (key: string) => void }

/** The month as a grid: a few chips per day, a count for the rest, and a popover that lists a whole day. */
export function MonthView({ agentId, tasks, anchor, now, onOpenDay }: Props) {
    const navigate = useNavigate()
    const here = useHere()
    const weeks = useMemo(() => monthGrid(tasks, anchor, now), [tasks, anchor, now])
    const [open, setOpen] = useState<{ key: string; anchor: HTMLElement } | null>(null)
    const anchorRef = useRef<HTMLElement | null>(null)
    anchorRef.current = open?.anchor ?? null
    const popRef = useRef<HTMLDivElement>(null)
    useDismiss(!!open, () => setOpen(null), popRef, anchorRef)

    const chip = (c: DayChip) => (
        <ScheduleChip
            key={c.key}
            state={c.state}
            time={chipTime(c)}
            title={c.task.title}
            repeat={chipRepeat(c)}
            onClick={() => navigate(taskHref(agentId, c.task, c.state), { state: here })}
        />
    )

    const days: MonthGridDay[][] = weeks.map((week) =>
        week.map((d) => ({
            key: d.key,
            day: d.date.getDate(),
            label: d.date.getDate() === 1 ? firstOfMonth.format(d.date) : undefined,
            today: d.key === dayKey(now),
            outside: d.outside,
            name: dayName.format(d.date),
            chips: d.chips.slice(0, FIT).map(chip),
            more: d.chips.length > FIT ? d.chips.length - FIT : undefined,
        }))
    )
    const openDay = weeks.flat().find((d) => d.key === open?.key)

    return (
        <>
            <MonthGrid
                label={new Intl.DateTimeFormat('en-US', { month: 'long', year: 'numeric' }).format(anchor)}
                weekdays={WEEKDAYS}
                weeks={days}
                onOpenDay={onOpenDay}
                onOpenMore={(key, el) => setOpen({ key, anchor: el })}
            />
            {open && openDay && (
                <Floating anchor={anchorRef} className="w-320 rounded-xl border border-border-strong bg-surface shadow-popover">
                    <div ref={popRef} role="dialog" aria-label={dayName.format(openDay.date)} className="flex flex-col">
                        <div className="flex items-center justify-between gap-12 border-b border-border px-16 py-12">
                            <div className="flex flex-col gap-2">
                                <span className="type-body-strong text-fg">{dayName.format(openDay.date)}</span>
                                <span className="type-mono-xs text-faint">{openDay.chips.length} scheduled</span>
                            </div>
                            <IconButton icon="close" variant="ghost" size="sm" aria-label="Close" onClick={() => setOpen(null)} />
                        </div>
                        <div className="flex max-h-[320px] flex-col gap-4 overflow-y-auto p-12">{openDay.chips.map(chip)}</div>
                        <div className="border-t border-border p-8">
                            <Button variant="ghost" trailingIcon="chevron-right" block onClick={() => onOpenDay(openDay.key)}>
                                Open day view
                            </Button>
                        </div>
                    </div>
                </Floating>
            )}
        </>
    )
}
