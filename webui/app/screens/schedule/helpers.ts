import type { Task } from '../../api/endpoints/tasks'
import type { ChipState, DayChip, Moment } from '../../lib/schedule'
import { clock, repeatText } from '../../lib/schedule'

/** A task's page; a moment that has a run on record opens that run. */
export function taskHref(agentId: string, task: Task, state?: ChipState): string {
    const base = `/${agentId}/tasks/${encodeURIComponent(task.slug)}`
    const known = state === 'answered' || state === 'running' || state === 'no-response' || state === 'interrupted'
    return known && task.last_run ? `${base}?run=${encodeURIComponent(task.last_run.run_id)}` : base
}

export const chipTime = (c: DayChip) => (c.kind === 'single' ? clock(c.at) : undefined)
export const chipRepeat = (c: DayChip) => (c.kind === 'repeat' ? repeatText(c) : undefined)

const STATE_WORDS: Record<ChipState, string> = {
    answered: 'Answered',
    running: 'Running',
    'no-response': 'No response',
    interrupted: 'Interrupted',
    upcoming: 'Scheduled',
    past: 'Ran',
    paused: 'Paused',
}
export const stateWords = (s: ChipState) => STATE_WORDS[s]

export const momentKey = (m: Moment) => `${m.task.slug}@${m.at.toISOString()}`
