import { api } from '../client'

/** A project as the server reports it: a folder mounted to one agent. */
export type Project = {
    id: string
    agent_id: string
    name: string
    /** The canonical path, without Windows' `\\?\` prefix. */
    folder: string
    available: boolean
    shell_enabled: boolean
    owner_id: string
    is_owner: boolean
    session_count: number
    created_at: number
}

/** One session of a project, as the project row lists it. */
export type ProjectSessionRow = {
    topic: string
    title: string
    is_thinking: boolean
    /** Unix milliseconds; null for a brand-new empty session. */
    last_activity: number | null
}

/** A project plus whether its instruction document exists; the text is never returned. */
export type ProjectDetail = Project & {
    instructions_present: boolean
    instructions_truncated: boolean
}

export type CreateProjectSession = {
    topic: string
    title: string
    channel_id: string
}

const base = (agentId: string) => `/agents/${encodeURIComponent(agentId)}/projects`

/** The channel id a project session lives on; the server owns the same prefix. */
export const projectChannel = (projectId: string) => `project__${projectId}`

export const listProjects = (agentId: string) => api.get<Project[]>(base(agentId))

/** Desktop only: the server refuses a browser principal with `403 desktop_only`. */
export const openProject = (agentId: string, folder: string) =>
    api.post<Project>(base(agentId), { folder })

export const getProject = (agentId: string, projectId: string) =>
    api.get<ProjectDetail>(`${base(agentId)}/${encodeURIComponent(projectId)}`)

export const updateProject = (
    agentId: string,
    projectId: string,
    body: { name?: string; shell_enabled?: boolean }
) => api.patch<Project>(`${base(agentId)}/${encodeURIComponent(projectId)}`, body)

export const removeProject = (agentId: string, projectId: string) =>
    api.delete<{ removed: boolean }>(`${base(agentId)}/${encodeURIComponent(projectId)}`)

/** The caller's own sessions in the project. */
export const listProjectSessions = (agentId: string, projectId: string) =>
    api.get<ProjectSessionRow[]>(`${base(agentId)}/${encodeURIComponent(projectId)}/sessions`)

export const createProjectSession = (agentId: string, projectId: string) =>
    api.post<CreateProjectSession>(`${base(agentId)}/${encodeURIComponent(projectId)}/sessions`)
