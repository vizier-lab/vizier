import { Button, FilterTabs, Status, Switch } from '@vizier/design'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useNavigate, useParams, useSearchParams } from 'react-router'
import { listAllTasks, updateTask, type Task } from '../api/endpoints/tasks'
import { describeCron } from '../lib/cron'
import { relativeTime } from '../lib/format'
import { personName, requesterText, runStatus, scheduleText, toInput } from '../lib/tasks'
import { useMobile } from '../lib/useMobile'
import { usePagedList } from '../lib/usePagedList'
import { useHere } from '../lib/returnTo'
import { countTasks, DEFAULT_SCHEDULE_FILTER, parseDayKey } from '../lib/schedule'
import { FILTER_LABELS } from '../screens/schedule/filters'
import { ScheduleCalendar, type Range } from '../screens/schedule/ScheduleCalendar'
import { PagedTable } from '../screens/shared/PagedTable'
import { PageFrame } from '../screens/shared/PageFrame'
import { toast } from '../stores/toasts'

const SORTS = [
    { value: 'next_run', label: 'Next run' },
    { value: 'name', label: 'Name' },
    { value: 'last_run', label: 'Last run' },
]

type View = 'calendar' | 'table'

/** Every task as the page's two views read them, on one agent: a calendar and a table. */
export default function Schedule() {
    const { agentId = '' } = useParams()
    const [params, setParams] = useSearchParams()
    const view: View = params.get('view') === 'table' ? 'table' : 'calendar'
    return view === 'table' ? (
        <ScheduleTable agentId={agentId} switcher={<ViewSwitch view={view} onChange={(v) => setView(setParams, v)} />} />
    ) : (
        <ScheduleCalendarPage agentId={agentId} switcher={<ViewSwitch view={view} onChange={(v) => setView(setParams, v)} />} />
    )
}

/** The view lives in the URL; what was narrowed (search, filter) carries over, the table's page and sort do not. */
function setView(setParams: ReturnType<typeof useSearchParams>[1], v: View) {
    setParams(
        (prev) => {
            const next = new URLSearchParams(prev)
            next.delete('page')
            next.delete('sort')
            next.delete('order')
            if (v === 'table') next.set('view', 'table')
            else next.delete('view')
            return next
        },
        { replace: true }
    )
}

function ViewSwitch({ view, onChange }: { view: View; onChange: (v: View) => void }) {
    return (
        <FilterTabs
            aria-label="Schedule view"
            value={view}
            onChange={(v) => onChange(v as View)}
            options={[
                { value: 'calendar', label: 'Calendar' },
                { value: 'table', label: 'Table' },
            ]}
        />
    )
}

function ScheduleCalendarPage({ agentId, switcher }: { agentId: string; switcher: React.ReactNode }) {
    const navigate = useNavigate()
    const here = useHere()
    const [params, setParams] = useSearchParams()
    const [tasks, setTasks] = useState<Task[] | null>(null)
    const [error, setError] = useState<Error | null>(null)
    const [now, setNow] = useState(() => new Date())

    useEffect(() => {
        const ctl = new AbortController()
        setTasks(null)
        listAllTasks(agentId, ctl.signal).then(
            (t) => {
                setTasks(t)
                setError(null)
            },
            (e) => e?.name !== 'AbortError' && setError(e)
        )
        return () => ctl.abort()
    }, [agentId])

    // The "now" line and what counts as ahead move once a minute.
    useEffect(() => {
        const id = setInterval(() => setNow(new Date()), 60_000)
        return () => clearInterval(id)
    }, [])

    const range: Range = params.get('range') === 'day' ? 'day' : 'month'
    const date = parseDayKey(params.get('date')) ? params.get('date') : null
    const filter = params.get('filter') ?? DEFAULT_SCHEDULE_FILTER
    const q = params.get('q') ?? ''

    const onChange = useCallback(
        (patch: { range?: Range; date?: string | null; q?: string; filter?: string }) =>
            setParams(
                (prev) => {
                    const next = new URLSearchParams(prev)
                    const put = (key: string, value: string | null | undefined, def: string | null) => {
                        if (value === undefined) return
                        if (value && value !== def) next.set(key, value)
                        else next.delete(key)
                    }
                    put('range', patch.range, 'month')
                    put('date', patch.date, null)
                    put('q', patch.q, '')
                    put('filter', patch.filter, DEFAULT_SCHEDULE_FILTER)
                    return next
                },
                { replace: true }
            ),
        [setParams]
    )

    const counts = useMemo(() => (tasks ? countTasks(tasks) : null), [tasks])
    return (
        <PageFrame
            title="Schedule"
            meta={counts ? `${counts.all} task${counts.all === 1 ? '' : 's'} · ${counts.active} active` : undefined}
            actions={
                <>
                    {switcher}
                    <Button variant="primary" icon="plus" onClick={() => navigate(`/${agentId}/tasks/new/edit`, { state: here })}>
                        New task
                    </Button>
                </>
            }
        >
            <ScheduleCalendar
                agentId={agentId}
                tasks={tasks}
                error={error}
                now={now}
                range={range}
                date={date}
                q={q}
                filter={filter}
                onChange={onChange}
                onCreate={() => navigate(`/${agentId}/tasks/new/edit`, { state: here })}
            />
        </PageFrame>
    )
}

function ScheduleTable({ agentId, switcher }: { agentId: string; switcher: React.ReactNode }) {
    const navigate = useNavigate()
    const here = useHere()
    const list = usePagedList<Task>(`/agents/${agentId}/tasks`, { sort: 'next_run', filter: DEFAULT_SCHEDULE_FILTER })
    // A phone keeps only the task, its schedule and its switch; the rest is on the task's page.
    const mobile = useMobile()
    const keep = <T,>(cells: T[]) => (mobile ? cells.filter((_, i) => i !== 2 && i !== 3) : cells)
    const create = () => navigate(`/${agentId}/tasks/new/edit`, { state: here })

    const toggle = async (task: Task, on: boolean) => {
        try {
            await updateTask(agentId, task.slug, toInput(task, { is_active: on }))
            list.reload()
        } catch (e) {
            toast.error("Couldn't change the task", e instanceof Error ? e.message : undefined)
        }
    }

    const counts = list.data?.counts
    return (
        <PageFrame
            title="Schedule"
            meta={counts ? `${counts.all ?? 0} task${counts.all === 1 ? '' : 's'} · ${counts.active ?? 0} active` : undefined}
            actions={
                <>
                    {switcher}
                    <Button variant="primary" icon="plus" onClick={create}>
                        New task
                    </Button>
                </>
            }
        >
            <PagedTable<Task>
                label="Tasks"
                defaultFilter={DEFAULT_SCHEDULE_FILTER}
                searchLabel="Search tasks"
                list={list}
                rowKey={(t) => t.slug}
                onSelect={(t) => navigate(`/${agentId}/tasks/${encodeURIComponent(t.slug)}`, { state: here })}
                sorts={SORTS}
                filters={(['active', 'last_run_failed', 'all'] as const).map((f) => ({ value: f, label: FILTER_LABELS[f] }))}
                columns={keep([
                    { label: 'Task' },
                    { label: 'Schedule', width: 220 },
                    { label: 'Requested by', width: 170 },
                    { label: 'Latest run', width: 190 },
                    { label: 'Active', width: 80, align: 'end' as const },
                ])}
                endAligned={[mobile ? 2 : 4]}
                row={(t) =>
                    keep([
                        <span key="t" className="flex min-w-0 flex-col">
                            <span className="truncate type-body-strong text-fg">{t.title}</span>
                            <span className="truncate type-small text-muted">{t.instruction.split('\n')[0]}</span>
                        </span>,
                        <span key="s" className="flex min-w-0 flex-col gap-2">
                            {'CronTask' in t.schedule && describeCron(t.schedule.CronTask) ? (
                                <>
                                    <span className="truncate type-small text-fg">{describeCron(t.schedule.CronTask)}</span>
                                    <span className="truncate type-mono-xs text-faint">{scheduleText(t.schedule)}</span>
                                </>
                            ) : (
                                <span className="truncate type-mono-xs text-muted">{scheduleText(t.schedule)}</span>
                            )}
                        </span>,
                        <span key="r" className="block min-w-0 truncate type-small text-muted" title={requesterText(t.requester)}>{'user' in t.requester ? personName(t.requester.user) : requesterText(t.requester)}</span>,
                        <Status key="l" state={runStatus(t.last_run)} font="sans" detail={t.last_run ? relativeTime(t.last_run.ran_at) : undefined} />,
                        <span key="a" onClick={(e) => e.stopPropagation()}>
                            <Switch checked={t.is_active} aria-label={`${t.title} active`} onCheckedChange={(on) => void toggle(t, on)} />
                        </span>,
                    ])
                }
                empty={{
                    icon: 'tasks',
                    title: 'No duties scheduled',
                    message: 'A task is an instruction the agent carries out on a schedule, or once.',
                    action: (
                        <Button variant="primary" icon="plus" onClick={create}>
                            New task
                        </Button>
                    ),
                }}
            />
        </PageFrame>
    )
}
