import { api } from '../client'
import type { ChatMessage } from '../frames'

export type TaskRunState = 'running' | 'answered' | 'no_response' | 'interrupted'

export interface TaskRun {
    run_id: string
    id: number
    ran_at: string
    finished_at?: string | null
    state: TaskRunState
    /** Only on the detail routes; `null` for a run that answered nothing. */
    response?: string | null
}

export type TaskSchedule = { CronTask: string } | { OneTimeTask: string }

export interface Task {
    slug: string
    requester: { user: string } | { agent: string }
    title: string
    instruction: string
    is_active: boolean
    schedule: TaskSchedule
    last_executed_at?: string | null
    timestamp: string
    last_run: TaskRun | null
    next_run?: string
}

export type ScheduleInput = { type: 'Cron'; expression: string } | { type: 'OneTime'; datetime: string }

export interface TaskInput {
    slug: string
    title: string
    instruction: string
    schedule: ScheduleInput
    is_active?: boolean
}

const base = (agentId: string) => `/agents/${agentId}/tasks`

export const getTask = (agentId: string, slug: string) => api.get<Task>(`${base(agentId)}/${slug}`)
export const createTask = (agentId: string, input: TaskInput) => api.post<Task>(base(agentId), input)
export const updateTask = (agentId: string, slug: string, input: TaskInput) => api.put<Task>(`${base(agentId)}/${slug}`, input)
export const deleteTask = (agentId: string, slug: string) => api.delete(`${base(agentId)}/${slug}`)

export interface TaskRuns {
    runs: TaskRun[]
    has_more: boolean
}

export const taskRuns = (agentId: string, slug: string, before?: TaskRun, limit = 20) => {
    const qs = new URLSearchParams({ limit: String(limit) })
    if (before) {
        qs.set('before', before.ran_at)
        qs.set('before_id', String(before.id))
    }
    return api.get<TaskRuns>(`${base(agentId)}/${slug}/runs?${qs}`)
}

/** The run's conversation; its last reply is the run's answer. */
export const runHistory = (agentId: string, slug: string, runId: string) =>
    api.get<ChatMessage[]>(`${base(agentId)}/${slug}/runs/${encodeURIComponent(runId)}/history?limit=200`)
