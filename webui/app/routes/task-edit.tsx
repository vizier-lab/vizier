import { Button, DateTimeField, FilterTabs, MarkdownEditor, Notice, Skeleton, TextField } from '@vizier/design'
import { useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router'
import { ApiError } from '../api/client'
import { createTask, getTask, updateTask, type Task } from '../api/endpoints/tasks'
import { describeCron } from '../lib/cron'
import { useCarry, useReturnTo } from '../lib/returnTo'
import { isCron, slugOf } from '../lib/tasks'
import { useMobile } from '../lib/useMobile'
import { PageFrame } from '../screens/shared/PageFrame'
import { toast } from '../stores/toasts'

const FORM = 'task-form'

/** Creating (`/tasks/new/edit`) or editing a task, on its own page: the instruction gets the room. */
export default function TaskEdit() {
    const { agentId = '', slug = '' } = useParams()
    const navigate = useNavigate()
    const mobile = useMobile()
    const creating = slug === 'new'
    const [task, setTask] = useState<Task | null>(null)

    const [title, setTitle] = useState('')
    const [instruction, setInstruction] = useState('')
    const [kind, setKind] = useState<'cron' | 'one_time'>('cron')
    const [cron, setCron] = useState('0 9 * * *')
    const [at, setAt] = useState<Date | null>(null)
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()

    useEffect(() => {
        if (creating) return
        getTask(agentId, slug)
            .then((t) => {
                setTask(t)
                setTitle(t.title)
                setInstruction(t.instruction)
                setKind(isCron(t.schedule) ? 'cron' : 'one_time')
                if (isCron(t.schedule)) setCron(t.schedule.CronTask)
                else setAt(new Date(t.schedule.OneTimeTask))
            })
            .catch((e) => toast.error("Couldn't load the task", e.message))
    }, [agentId, slug, creating])

    const finalSlug = creating ? slugOf(title) : slug
    const valid = title.trim() !== '' && instruction.trim() !== '' && finalSlug !== '' && (kind === 'cron' ? cron.trim() !== '' : at !== null && at > new Date())
    const carry = useCarry()
    const fromList = useReturnTo(`/${agentId}/tasks`)
    const back = creating ? fromList : `/${agentId}/tasks/${encodeURIComponent(slug)}`

    const submit = async () => {
        if (!valid || busy) return
        setBusy(true)
        setError(undefined)
        const input = {
            slug: finalSlug,
            title: title.trim(),
            instruction: instruction.trim(),
            schedule: kind === 'cron' ? { type: 'Cron' as const, expression: cron.trim() } : { type: 'OneTime' as const, datetime: at!.toISOString() },
            is_active: task?.is_active ?? true,
        }
        try {
            if (creating) await createTask(agentId, input)
            else await updateTask(agentId, slug, input)
            toast.success(creating ? 'Task created' : 'Task saved')
            navigate(`/${agentId}/tasks/${encodeURIComponent(finalSlug)}`, { state: carry })
        } catch (e) {
            setError(e instanceof ApiError || e instanceof Error ? e.message : 'Something went wrong')
        } finally {
            setBusy(false)
        }
    }

    const human = kind === 'cron' ? describeCron(cron.trim()) : null
    return (
        <PageFrame
            fill
            back={{ href: back, label: creating ? 'Schedule' : (task?.title ?? 'Task'), state: creating ? undefined : carry }}
            title={creating ? 'New task' : `Edit ${task?.title ?? 'task'}`}
            meta={finalSlug ? `${finalSlug}${creating ? '' : ' · the slug is fixed once created'}` : 'An instruction the agent carries out on a schedule, or once'}
            actions={
                <>
                    <Button variant="ghost" onClick={() => navigate(back, { state: creating ? undefined : carry })}>
                        Cancel
                    </Button>
                    <Button type="submit" form={FORM} variant="primary" loading={busy} disabled={!valid}>
                        {creating ? 'Create task' : 'Save task'}
                    </Button>
                </>
            }
        >
            {!creating && !task ? (
                <Skeleton lines={10} className="m-24" />
            ) : (
                <form
                    id={FORM}
                    className="flex min-h-0 flex-1 flex-col gap-20 overflow-y-auto px-16 py-20 sm:px-24 sm:py-24"
                    onSubmit={(e) => {
                        e.preventDefault()
                        void submit()
                    }}
                >
                    {error && <Notice tone="failed" title="Couldn't save the task">{error}</Notice>}
                    <div className="grid grid-cols-1 items-start gap-16 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
                        <TextField label="Title" value={title} onChange={(e) => setTitle(e.target.value)} autoFocus={creating} />
                        <div className="flex flex-col items-start gap-12 [&>:last-child]:self-stretch">
                            <FilterTabs
                                aria-label="Schedule"
                                value={kind}
                                onChange={setKind}
                                options={[
                                    { value: 'cron', label: 'Repeats' },
                                    { value: 'one_time', label: 'Once' },
                                ]}
                            />
                            {kind === 'cron' ? (
                                <TextField
                                    label="Cron expression"
                                    mono
                                    value={cron}
                                    onChange={(e) => setCron(e.target.value)}
                                    helper={human ?? 'Minute, hour, day of month, month, day of week. 0 9 * * 1 is Mondays at 09:00.'}
                                />
                            ) : (
                                <DateTimeField label="Run at" value={at} onChange={setAt} min={new Date()} helper="Your time zone" />
                            )}
                        </div>
                    </div>
                    <div className="flex min-h-[360px] flex-1 flex-col gap-8">
                        <span className="type-ui-medium text-fg">Instruction</span>
                        <span className="-mt-4 type-small text-muted">What the agent is asked to do each time it runs.</span>
                        <MarkdownEditor look="compact" mode={mobile ? 'markdown' : undefined} value={instruction} onChange={setInstruction} onSaveShortcut={() => void submit()} className="min-h-0 flex-1" />
                    </div>
                </form>
            )}
        </PageFrame>
    )
}
