import { Button, Status, Switch } from '@vizier/design'
import { useNavigate, useParams } from 'react-router'
import { updateTask, type Task } from '../api/endpoints/tasks'
import { describeCron } from '../lib/cron'
import { relativeTime } from '../lib/format'
import { requesterText, runStatus, scheduleText, toInput } from '../lib/tasks'
import { useMobile } from '../lib/useMobile'
import { usePagedList } from '../lib/usePagedList'
import { PagedTable } from '../screens/shared/PagedTable'
import { PageFrame } from '../screens/shared/PageFrame'
import { toast } from '../stores/toasts'

const SORTS = [
    { value: 'next_run', label: 'Next run' },
    { value: 'name', label: 'Name' },
    { value: 'last_run', label: 'Last run' },
]

/** Every task the agent carries out, across the page. A task opens on its own page. */
export default function Tasks() {
    const { agentId = '' } = useParams()
    const navigate = useNavigate()
    const list = usePagedList<Task>(`/agents/${agentId}/tasks`, { sort: 'next_run' })
    // A phone keeps only the task, its schedule and its switch; the rest is on the task's page.
    const mobile = useMobile()
    const keep = <T,>(cells: T[]) => (mobile ? cells.filter((_, i) => i !== 2 && i !== 3) : cells)
    const create = () => navigate(`/${agentId}/tasks/new/edit`)

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
            title="Tasks"
            meta={counts ? `${counts.all ?? 0} task${counts.all === 1 ? '' : 's'} · ${counts.active ?? 0} active` : undefined}
            actions={
                <Button variant="primary" icon="plus" onClick={create}>
                    New task
                </Button>
            }
        >
            <PagedTable<Task>
                label="Tasks"
                searchLabel="Search tasks"
                list={list}
                rowKey={(t) => t.slug}
                onSelect={(t) => navigate(`/${agentId}/tasks/${encodeURIComponent(t.slug)}`)}
                sorts={SORTS}
                filters={[
                    { value: 'all', label: 'All' },
                    { value: 'active', label: 'Active' },
                    { value: 'last_run_failed', label: 'Last run failed' },
                ]}
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
                        <span key="r" className="truncate type-small text-muted">{requesterText(t.requester)}</span>,
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
