import { Button, EmptyState, KeyValueList, LoadMore, PanelHeader, RunRow, Skeleton, Status, Switch } from '@vizier/design'
import { Enter, stagger } from '../lib/motion'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useNavigate, useParams, useSearchParams } from 'react-router'
import { ApiError } from '../api/client'
import { deleteTask, getTask, runHistory, taskRuns, updateTask, type Task, type TaskRun } from '../api/endpoints/tasks'
import { describeCron } from '../lib/cron'
import { absoluteTime } from '../lib/format'
import { replyText } from '../lib/messages'
import { requesterText, runDuration, runStatus, scheduleText, toInput } from '../lib/tasks'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { Markdown } from '../screens/shared/Markdown'
import { PageFrame } from '../screens/shared/PageFrame'
import { Panes } from '../screens/shared/Panes'
import { toast } from '../stores/toasts'

/** One task: what it asks and what each run answered, with its details and runs beside it. */
export default function TaskPage() {
    const { agentId = '', slug = '' } = useParams()
    const navigate = useNavigate()
    const [params, setParams] = useSearchParams()
    const [task, setTask] = useState<Task | null>(null)
    const [missing, setMissing] = useState(false)
    const [runs, setRuns] = useState<TaskRun[]>([])
    const [more, setMore] = useState(false)
    const [loading, setLoading] = useState(true)
    const [deleting, setDeleting] = useState(false)
    const [answer, setAnswer] = useState<string | null | undefined>(undefined)

    useEffect(() => {
        getTask(agentId, slug).then(setTask, (e) => {
            if (e instanceof ApiError && e.status === 404) setMissing(true)
            else toast.error("Couldn't load the task", e instanceof Error ? e.message : undefined)
        })
    }, [agentId, slug])

    const load = useCallback(
        async (after?: TaskRun) => {
            setLoading(true)
            try {
                const page = await taskRuns(agentId, slug, after)
                setRuns((r) => (after ? [...r, ...page.runs] : page.runs))
                setMore(page.has_more)
            } catch {
                /* the runs pane says it has none */
            } finally {
                setLoading(false)
            }
        },
        [agentId, slug]
    )
    useEffect(() => void load(), [load])

    // The run being read: the one asked for, else the latest.
    const runRef = useRef<HTMLElement>(null)
    const asked = params.get('run')
    const open = runs.find((r) => r.run_id === asked) ?? runs[0] ?? null
    useEffect(() => {
        if (!open) return
        setAnswer(undefined)
        let live = true
        runHistory(agentId, slug, open.run_id)
            .then((entries) => {
                if (!live) return
                const last = [...entries].reverse().find((e) => e.content.Response && replyText(e.content.Response.content))
                setAnswer(last ? replyText(last.content.Response!.content) : null)
            })
            .catch(() => live && setAnswer(null))
        return () => {
            live = false
        }
    }, [open, agentId, slug])

    const toggle = async (on: boolean) => {
        if (!task) return
        try {
            await updateTask(agentId, slug, toInput(task, { is_active: on }))
            setTask(await getTask(agentId, slug))
        } catch (e) {
            toast.error("Couldn't change the task", e instanceof Error ? e.message : undefined)
        }
    }

    const remove = async () => {
        try {
            await deleteTask(agentId, slug)
            toast.success('Task deleted')
            navigate(`/${agentId}/tasks`)
        } catch (e) {
            setDeleting(false)
            toast.error("Couldn't delete the task", e instanceof Error ? e.message : undefined)
        }
    }

    const back = { href: `/${agentId}/tasks`, label: 'Tasks' }
    if (missing) {
        return (
            <PageFrame title={slug} back={back}>
                <EmptyState kind="error" title="This task isn’t there" message="It may have been deleted. Every task is listed on Tasks." />
            </PageFrame>
        )
    }

    const cron = task && 'CronTask' in task.schedule ? describeCron(task.schedule.CronTask) : null
    return (
        <PageFrame
            fill
            back={back}
            title={task?.title ?? slug}
            meta={task ? `${cron ?? scheduleText(task.schedule)}${task.next_run ? ` · next ${absoluteTime(task.next_run)}` : ''}` : undefined}
            actions={
                task && (
                    <>
                        <label className="flex items-center gap-8 type-ui text-muted">
                            <Switch checked={task.is_active} aria-label="Active" onCheckedChange={(on) => void toggle(on)} />
                            {task.is_active ? 'Active' : 'Paused'}
                        </label>
                        <Button variant="ghost" icon="trash" onClick={() => setDeleting(true)}>
                            Delete
                        </Button>
                        <Button variant="primary" icon="pencil" onClick={() => navigate(`/${agentId}/tasks/${encodeURIComponent(slug)}/edit`)}>
                            Edit
                        </Button>
                    </>
                )
            }
        >
            <Panes
                paneWidth={360}
                paneLabel="Details and runs"
                pane={
                    <>
                        {task ? (
                            <KeyValueList
                                items={[
                                    {
                                        label: 'Schedule',
                                        value: cron ? (
                                            <span className="flex flex-col items-end gap-2">
                                                <span>{cron}</span>
                                                <span className="type-mono-xs text-faint">{scheduleText(task.schedule)}</span>
                                            </span>
                                        ) : (
                                            <span className="type-mono-sm">{scheduleText(task.schedule)}</span>
                                        ),
                                    },
                                    { label: 'Next run', value: task.next_run ? absoluteTime(task.next_run) : '–' },
                                    { label: 'Requested by', value: requesterText(task.requester) },
                                    { label: 'Created', value: absoluteTime(task.timestamp) },
                                ]}
                            />
                        ) : (
                            <Skeleton lines={4} />
                        )}
                        <section className="flex flex-col gap-8">
                            <PanelHeader title="Runs" meta={runs.length ? `${runs.length}${more ? '+' : ''}` : undefined} level={3} />
                            {runs.length === 0 && !loading && <p className="type-small text-faint">It hasn’t run yet. Each run is listed here with what the agent answered.</p>}
                            <div className="flex flex-col gap-2">
                                {runs.map((r, i) => (
                                    <Enter key={r.id} delay={stagger(i, 0.03)}>
                                        <RunRow
                                            time={absoluteTime(r.ran_at)}
                                            state={runStatus(r) as never}
                                            duration={runDuration(r)}
                                            selected={open?.id === r.id}
                                            onSelect={() => {
                                                setParams({ run: r.run_id }, { replace: true })
                                                runRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' })
                                            }}
                                        />
                                    </Enter>
                                ))}
                            </div>
                            {more && <LoadMore label="Load older" loading={loading} onLoad={() => void load(runs[runs.length - 1])} />}
                        </section>
                    </>
                }
            >
                <section className="flex max-w-[72ch] flex-col gap-8">
                    <PanelHeader title="Instruction" level={2} />
                    {task ? <Markdown size="document">{task.instruction}</Markdown> : <Skeleton lines={3} />}
                </section>

                <section ref={runRef} className="flex max-w-[72ch] scroll-mt-24 flex-col gap-8 border-t border-border pt-24">
                    {open ? (
                        <>
                            <div className="flex flex-wrap items-center gap-12">
                                <PanelHeader title={open === runs[0] ? 'Latest run' : 'Run'} level={2} className="flex-1" />
                                <Status state={runStatus(open)} font="sans" detail={`${absoluteTime(open.ran_at)} · ${runDuration(open)}`} />
                            </div>
                            {answer === undefined ? (
                                <Skeleton lines={4} />
                            ) : answer ? (
                                <Markdown size="document">{answer}</Markdown>
                            ) : (
                                <p className="type-small text-muted">This run answered nothing.</p>
                            )}
                        </>
                    ) : (
                        !loading && (
                            <EmptyState
                                kind="empty"
                                icon="tasks"
                                title="Hasn’t run yet"
                                message={task?.is_active === false ? 'It is paused: switch it on to let it run.' : task?.next_run ? `Its first run is ${absoluteTime(task.next_run)}.` : 'What it answers will be here.'}
                            />
                        )
                    )}
                </section>
            </Panes>

            <ConfirmDialog
                open={deleting}
                title={`Delete ${task?.title ?? 'task'}?`}
                removes={['The task and its schedule', 'Its run history']}
                confirmLabel="Delete task"
                onConfirm={() => void remove()}
                onCancel={() => setDeleting(false)}
            />
        </PageFrame>
    )
}
