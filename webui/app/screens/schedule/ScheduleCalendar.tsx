import { Button, EmptyState, FilterTabs, IconButton, SearchField, Skeleton } from '@vizier/design'
import { useMemo } from 'react'
import type { Task } from '../../api/endpoints/tasks'
import { count } from '../../lib/format'
import {
    addDays,
    addMonths,
    countTasks,
    dayKey,
    filterTasks,
    parseDayKey,
    startOfDay,
    type ScheduleFilter,
} from '../../lib/schedule'
import { useMobile } from '../../lib/useMobile'
import { Agenda } from './Agenda'
import { DayViewPane } from './DayViewPane'
import { FILTER_LABELS } from './filters'
import { MonthView } from './MonthView'

export type Range = 'month' | 'day'

type Props = {
    agentId: string
    tasks: Task[] | null
    error: Error | null
    now: Date
    range: Range
    /** The day the view is on, as "2026-10-30". */
    date: string | null
    q: string
    filter: ScheduleFilter | string
    onChange: (patch: { range?: Range; date?: string | null; q?: string; filter?: string }) => void
    onCreate: () => void
}

const monthTitle = new Intl.DateTimeFormat('en-US', { month: 'long', year: 'numeric' })
const dayTitle = new Intl.DateTimeFormat('en-US', { weekday: 'long', month: 'long', day: 'numeric' })

/** The calendar half of the Schedule page: month or day, over the same filter and search as the table. */
export function ScheduleCalendar({ agentId, tasks, error, now, range, date, q, filter, onChange, onCreate }: Props) {
    const mobile = useMobile()
    const day = parseDayKey(date) ?? startOfDay(now)
    const todayKey = dayKey(now)
    const counts = useMemo(() => (tasks ? countTasks(tasks) : null), [tasks])
    const shown = useMemo(() => (tasks ? filterTasks(tasks, filter, q) : []), [tasks, filter, q])

    const step = (by: -1 | 1) =>
        onChange({ date: dayKey(range === 'month' ? addMonths(day, by) : addDays(day, by)) })
    const openDay = (key: string) => onChange({ range: 'day', date: key })

    return (
        <div className="flex flex-col gap-16">
            <div className="flex flex-wrap items-center justify-between gap-12">
                <div className="flex flex-wrap items-center gap-10">
                    <span className="flex gap-4">
                        <IconButton icon="chevron-left" aria-label={range === 'month' ? 'Previous month' : 'Previous day'} onClick={() => step(-1)} />
                        <IconButton icon="chevron-right" aria-label={range === 'month' ? 'Next month' : 'Next day'} onClick={() => step(1)} />
                    </span>
                    <Button variant="secondary" onClick={() => onChange({ date: null })} disabled={dayKey(day) === todayKey && range === 'day'}>
                        Today
                    </Button>
                    <h2 className="type-heading text-fg">{(range === 'month' ? monthTitle : dayTitle).format(day)}</h2>
                </div>
                <div className="flex flex-wrap items-center gap-10">
                    <div className="w-200">
                        <SearchField aria-label="Search tasks" placeholder="Search tasks" value={q} onValueChange={(v) => onChange({ q: v })} />
                    </div>
                    <FilterTabs
                        aria-label="Filter tasks"
                        value={filter}
                        onChange={(v) => onChange({ filter: v })}
                        options={(['active', 'last_run_failed', 'all'] as const).map((f) => ({
                            value: f,
                            label: FILTER_LABELS[f],
                            count: counts ? count(counts[f]) : undefined,
                        }))}
                    />
                    <FilterTabs
                        aria-label="Calendar range"
                        value={range}
                        onChange={(v) => onChange({ range: v as Range })}
                        options={[
                            { value: 'month', label: 'Month' },
                            { value: 'day', label: 'Day' },
                        ]}
                    />
                </div>
            </div>
            {error ? (
                <EmptyState kind="error" title="Couldn't load the schedule" message={error.message} />
            ) : !tasks ? (
                <Skeleton lines={6} />
            ) : tasks.length === 0 ? (
                <EmptyState
                    kind="empty"
                    icon="calendar"
                    title="No duties scheduled"
                    message="A task is an instruction the agent carries out on a schedule, or once."
                    action={
                        <Button variant="primary" icon="plus" onClick={onCreate}>
                            New task
                        </Button>
                    }
                />
            ) : shown.length === 0 ? (
                <EmptyState kind="no-results" title="Nothing matched" message="Try a different search or filter." />
            ) : range === 'month' ? (
                mobile ? (
                    <Agenda agentId={agentId} tasks={shown} anchor={day} now={now} onOpenDay={openDay} />
                ) : (
                    <MonthView agentId={agentId} tasks={shown} anchor={day} now={now} onOpenDay={openDay} />
                )
            ) : (
                <DayViewPane agentId={agentId} tasks={shown} day={day} now={now} onOpenDay={(k) => onChange({ date: k })} />
            )}
        </div>
    )
}
