import { Badge, Button, Dialog, EmptyState, KeyValueList, LoadMore, PanelHeader, RunRow, Status, Switch } from '@vizier/design'
import { useCallback, useEffect, useState } from 'react'
import { useParams, useSearchParams } from 'react-router'
import { ApiError } from '../api/client'
import {
    createTask,
    deleteTask,
    getTask,
    runHistory,
    taskRuns,
    updateTask,
    type Task,
    type TaskInput,
    type TaskRun,
} from '../api/endpoints/tasks'
import { absoluteTime, relativeTime } from '../lib/format'
import { replyText } from '../lib/messages'
import { isCron, requesterText, runDuration, runStatus, scheduleText } from '../lib/tasks'
import { usePagedList } from '../lib/usePagedList'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { Markdown } from '../screens/shared/Markdown'
import { PagedTable } from '../screens/shared/PagedTable'
import { describeCron } from '../lib/cron'
import { SplitView } from '../screens/shared/SplitView'
import { useWide } from '../lib/useWide'
import { PageFrame } from '../screens/shared/PageFrame'
import { TaskDialog } from '../screens/tasks/TaskDialog'
import { toast } from '../stores/toasts'

const SORTS = [
    { value: 'next_run', label: 'Next run' },
    { value: 'name', label: 'Name' },
    { value: 'last_run', label: 'Last run' },
]

export default function Tasks() {
    const { agentId = '' } = useParams()
    const [params, setParams] = useSearchParams()
    const list = usePagedList<Task>(`/agents/${agentId}/tasks`, { sort: 'next_run' })
    const selectedSlug = params.get('task')
    const selected = list.data?.items.find((t) => t.slug === selectedSlug) ?? null

    const [editing, setEditing] = useState<Task | 'new' | null>(null)
    const [deleting, setDeleting] = useState<Task | null>(null)
    const [saving, setSaving] = useState(false)
    const [error, setError] = useState<string>()

    const select = (slug: string | null) =>
        setParams(
            (p) => {
                const next = new URLSearchParams(p)
                if (slug) next.set('task', slug)
                else next.delete('task')
                next.delete('run')
                return next
            },
            { replace: true }
        )

    // Beside the list the detail is always there: the first task until another is chosen.
    const wide = useWide()
    useEffect(() => {
        const first = list.data?.items[0]
        if (wide && !selectedSlug && first) select(first.slug)
    }, [wide, selectedSlug, list.data]) // eslint-disable-line react-hooks/exhaustive-deps

    // With the detail beside it the list drops "Requested by", which the detail has.
    const split = wide && !!selected
    const keep = <T,>(cells: T[]) => (split ? cells.filter((_, i) => i !== 2) : cells)

    const submit = async (input: TaskInput) => {
        setSaving(true)
        setError(undefined)
        try {
            if (editing === 'new') await createTask(agentId, input)
            else if (editing) await updateTask(agentId, editing.slug, input)
            setEditing(null)
            list.reload()
            toast.success('Task saved')
        } catch (e) {
            setError(e instanceof ApiError || e instanceof Error ? e.message : 'Something went wrong')
        } finally {
            setSaving(false)
        }
    }

    const toggle = async (task: Task, on: boolean) => {
        try {
            await updateTask(agentId, task.slug, {
                slug: task.slug,
                title: task.title,
                instruction: task.instruction,
                schedule: isCron(task.schedule)
                    ? { type: 'Cron', expression: task.schedule.CronTask }
                    : { type: 'OneTime', datetime: task.schedule.OneTimeTask },
                is_active: on,
            })
            list.reload()
        } catch (e) {
            toast.error("Couldn't change the task", e instanceof Error ? e.message : undefined)
        }
    }

    const remove = async () => {
        if (!deleting) return
        try {
            await deleteTask(agentId, deleting.slug)
            toast.success('Task deleted')
            if (selectedSlug === deleting.slug) select(null)
            setDeleting(null)
            list.reload()
        } catch (e) {
            toast.error("Couldn't delete the task", e instanceof Error ? e.message : undefined)
        }
    }

    const counts = list.data?.counts
    return (
        <PageFrame
            title="Tasks"
            meta={counts ? `${counts.all ?? 0} task${counts.all === 1 ? '' : 's'} · ${counts.active ?? 0} active` : undefined}
            actions={
                <Button variant="primary" icon="plus" onClick={() => (setError(undefined), setEditing('new'))}>
                    New task
                </Button>
            }
        >
            <SplitView
                title={selected?.title ?? ''}
                onClose={() => select(null)}
                detail={selected ? <TaskDetail agentId={agentId} task={selected} onEdit={() => (setError(undefined), setEditing(selected))} onDelete={() => setDeleting(selected)} /> : undefined}
            >
            <PagedTable<Task>
                label="Tasks"
                searchLabel="Search tasks"
                list={list}
                rowKey={(t) => t.slug}
                selectedKey={selectedSlug ?? undefined}
                onSelect={(t) => select(t.slug)}
                sorts={SORTS}
                filters={[
                    { value: 'all', label: 'All' },
                    { value: 'active', label: 'Active' },
                    { value: 'last_run_failed', label: 'Last run failed' },
                ]}
                columns={keep([
                    { label: 'Task' },
                    { label: 'Schedule', width: 180 },
                    { label: 'Requested by', width: 150 },
                    { label: 'Latest run', width: 170 },
                    { label: 'Active', width: 80, align: 'end' as const },
                ])}
                endAligned={split ? [3] : [4]}
                row={(t) => keep([
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
                    <span key="r" className="truncate type-small text-muted">{requesterText(t.requester)}</span>,
                    <Status key="l" state={runStatus(t.last_run)} font="sans" detail={t.last_run ? relativeTime(t.last_run.ran_at) : undefined} />,
                    <span key="a" onClick={(e) => e.stopPropagation()}>
                        <Switch checked={t.is_active} aria-label={`${t.title} active`} onCheckedChange={(on) => void toggle(t, on)} />
                    </span>,
                ])}
                empty={{
                    icon: 'tasks',
                    title: 'No duties scheduled',
                    message: 'A task is an instruction the agent carries out on a schedule, or once.',
                    action: (
                        <Button variant="primary" icon="plus" onClick={() => setEditing('new')}>
                            New task
                        </Button>
                    ),
                }}
            />
            </SplitView>

            <Dialog
                open={editing !== null}
                title={editing === 'new' ? 'New task' : 'Edit task'}
                confirmLabel=""
                onConfirm={() => {}}
                onCancel={() => setEditing(null)}
                size="lg"
            >
                {editing !== null && (
                    <TaskDialog task={editing === 'new' ? undefined : editing} busy={saving} error={error} onSubmit={(i) => void submit(i)} onCancel={() => setEditing(null)} />
                )}
            </Dialog>

            <ConfirmDialog
                open={deleting !== null}
                title={`Delete ${deleting?.title ?? 'task'}?`}
                removes={['The task and its schedule', 'Its run history']}
                confirmLabel="Delete task"
                onConfirm={() => void remove()}
                onCancel={() => setDeleting(null)}
            />
        </PageFrame>
    )
}

function TaskDetail({ agentId, task, onEdit, onDelete }: { agentId: string; task: Task; onEdit: () => void; onDelete: () => void }) {
    const [runs, setRuns] = useState<TaskRun[]>([])
    const [more, setMore] = useState(false)
    const [loading, setLoading] = useState(false)
    const [open, setOpen] = useState<TaskRun | null>(null)
    const [answer, setAnswer] = useState<string | null | undefined>(undefined)

    const load = useCallback(
        async (reset: boolean) => {
            setLoading(true)
            try {
                const page = await taskRuns(agentId, task.slug, reset ? undefined : runs[runs.length - 1])
                setRuns((r) => (reset ? page.runs : [...r, ...page.runs]))
                setMore(page.has_more)
            } finally {
                setLoading(false)
            }
        },
        [agentId, task.slug, runs]
    )

    useEffect(() => {
        setRuns([])
        setOpen(null)
        void load(true)
    }, [task.slug]) // eslint-disable-line react-hooks/exhaustive-deps

    useEffect(() => {
        if (!open) return
        setAnswer(undefined)
        let alive = true
        runHistory(agentId, task.slug, open.run_id)
            .then((entries) => {
                if (!alive) return
                const last = [...entries].reverse().find((e) => e.content.Response && replyText(e.content.Response.content))
                setAnswer(last ? replyText(last.content.Response!.content) : null)
            })
            .catch(() => alive && setAnswer(null))
        return () => {
            alive = false
        }
    }, [open, agentId, task.slug])

    return (
        <div className="flex flex-col gap-24">
            <div className="flex items-center gap-8">
                <Badge tone={task.is_active ? 'live' : 'neutral'}>{task.is_active ? 'Active' : 'Inactive'}</Badge>
                <span className="flex-1" />
                <Button size="sm" variant="secondary" onClick={onEdit}>Edit</Button>
                <Button size="sm" variant="destructive" onClick={onDelete}>Delete</Button>
            </div>
            <KeyValueList
                items={[
                    { label: 'Schedule', value: <span className="type-mono-sm">{scheduleText(task.schedule)}</span> },
                    { label: 'Next run', value: task.next_run ? absoluteTime(task.next_run) : '–' },
                    { label: 'Requested by', value: requesterText(task.requester) },
                    { label: 'Created', value: absoluteTime(task.timestamp) },
                ]}
            />
            <section className="flex flex-col gap-8">
                <PanelHeader title="Instruction" level={3} />
                <Markdown>{task.instruction}</Markdown>
            </section>
            <section className="flex flex-col gap-8">
                <PanelHeader title="Runs" meta={runs.length ? `${runs.length}${more ? '+' : ''}` : undefined} level={3} />
                {runs.length === 0 && !loading && <EmptyState kind="empty" icon="tasks" title="Hasn't run yet" />}
                {runs.map((r) => (
                    <RunRow key={r.id} time={absoluteTime(r.ran_at)} state={runStatus(r) as never} duration={runDuration(r)} selected={open?.id === r.id} onSelect={() => setOpen(r)} />
                ))}
                {more && <LoadMore label="Load older" loading={loading} onLoad={() => void load(false)} />}
            </section>
            {open && (
                <section className="flex flex-col gap-8">
                    <PanelHeader title={`Run ${absoluteTime(open.ran_at)}`} level={3} />
                    {answer === undefined ? <p className="type-small text-muted">Loading…</p> : answer ? <Markdown>{answer}</Markdown> : <p className="type-small text-muted">This run answered nothing.</p>}
                </section>
            )}
        </div>
    )
}
