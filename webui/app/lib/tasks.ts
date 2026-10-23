import type { Task, TaskInput, TaskRun, TaskSchedule } from '../api/endpoints/tasks'

export const isCron = (s: TaskSchedule): s is { CronTask: string } => 'CronTask' in s

/** "0 9 * * 1", or the one-time moment. */
export const scheduleText = (s: TaskSchedule) => (isCron(s) ? s.CronTask : new Date(s.OneTimeTask).toLocaleString())

export function requesterText(r: Task['requester']): string {
    return 'user' in r ? r.user : `${r.agent} (itself)`
}

type RunStatus = 'answered' | 'running' | 'no-response' | 'not-run' | 'interrupted'

/** The library's status for a run, or `not-run` for a task that never ran. */
export function runStatus(run: TaskRun | null | undefined): RunStatus {
    if (!run) return 'not-run'
    return run.state === 'no_response' ? 'no-response' : run.state
}

/** "41s", "1m 02s", or "–" while it has no end. */
export function runDuration(run: Pick<TaskRun, 'ran_at' | 'finished_at'>): string {
    if (!run.finished_at) return '–'
    const s = Math.max(0, Math.round((Date.parse(run.finished_at) - Date.parse(run.ran_at)) / 1000))
    return s < 60 ? `${s}s` : `${Math.floor(s / 60)}m ${String(s % 60).padStart(2, '0')}s`
}

/** A URL-safe slug from a title: lower-case words joined by dashes. */
export function slugOf(title: string): string {
    return title
        .toLowerCase()
        .normalize('NFKD')
        .replace(/[^a-z0-9]+/g, '-')
        .replace(/^-+|-+$/g, '')
        .slice(0, 60)
}

/** A task as the input that saves it again, with some of it changed. */
export function toInput(task: Task, patch: Partial<TaskInput> = {}): TaskInput {
    return {
        slug: task.slug,
        title: task.title,
        instruction: task.instruction,
        schedule: isCron(task.schedule) ? { type: 'Cron', expression: task.schedule.CronTask } : { type: 'OneTime', datetime: task.schedule.OneTimeTask },
        is_active: task.is_active,
        ...patch,
    }
}
