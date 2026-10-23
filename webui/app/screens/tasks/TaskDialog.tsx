import { Button, DateTimeField, FilterTabs, Notice, TextField, Textarea } from '@vizier/design'
import { useState } from 'react'
import type { Task, TaskInput } from '../../api/endpoints/tasks'
import { isCron, slugOf } from '../../lib/tasks'

type Props = {
    /** Editing this task; absent creates one. */
    task?: Task
    busy: boolean
    error?: string
    onSubmit: (input: TaskInput) => void
    onCancel: () => void
}

/** Create or edit a task: cron or one-time, with the instruction the agent is given. */
export function TaskDialog({ task, busy, error, onSubmit, onCancel }: Props) {
    const [title, setTitle] = useState(task?.title ?? '')
    const [instruction, setInstruction] = useState(task?.instruction ?? '')
    const [kind, setKind] = useState<'cron' | 'one_time'>(task ? (isCron(task.schedule) ? 'cron' : 'one_time') : 'cron')
    const [cron, setCron] = useState(task && isCron(task.schedule) ? task.schedule.CronTask : '0 9 * * *')
    const [at, setAt] = useState<Date | null>(task && !isCron(task.schedule) ? new Date(task.schedule.OneTimeTask) : null)

    const slug = task?.slug ?? slugOf(title)
    const valid = title.trim() !== '' && instruction.trim() !== '' && slug !== '' && (kind === 'cron' ? cron.trim() !== '' : at !== null && at > new Date())

    return (
        <form
            className="flex flex-col gap-16"
            onSubmit={(e) => {
                e.preventDefault()
                if (!valid) return
                onSubmit({
                    slug,
                    title: title.trim(),
                    instruction: instruction.trim(),
                    schedule: kind === 'cron' ? { type: 'Cron', expression: cron.trim() } : { type: 'OneTime', datetime: at!.toISOString() },
                    is_active: task?.is_active ?? true,
                })
            }}
        >
            {error && <Notice tone="failed" title="Couldn't save the task">{error}</Notice>}
            <TextField label="Title" value={title} onChange={(e) => setTitle(e.target.value)} helper={task ? undefined : slug ? `Slug: ${slug}` : undefined} autoFocus />
            <Textarea label="Instruction" rows={4} value={instruction} onChange={(e) => setInstruction(e.target.value)} helper="What the agent is asked to do each time it runs." />
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
                <TextField label="Cron expression" mono value={cron} onChange={(e) => setCron(e.target.value)} helper="Minute, hour, day of month, month, day of week. Example: 0 9 * * 1 is Mondays at 09:00." />
            ) : (
                <DateTimeField label="Run at" value={at} onChange={setAt} min={new Date()} helper="Your time zone" />
            )}
            <div className="flex justify-end gap-8">
                <Button type="button" variant="ghost" onClick={onCancel}>
                    Cancel
                </Button>
                <Button type="submit" variant="primary" loading={busy} disabled={!valid}>
                    {task ? 'Save task' : 'Create task'}
                </Button>
            </div>
        </form>
    )
}
