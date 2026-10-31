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
    /** The chat's saved Plan/Build mode (spec 041). */
    mode?: 'plan' | 'build'
    worktree?: { id: string; name: string; branch: string; available: boolean } | null
}

/** A project plus whether its instruction document exists; the text is never returned. */
export type ProjectDetail = Project & {
    instructions_present: boolean
    instructions_truncated: boolean
    /** Worktree folders that stay on disk when the project is removed (spec 041 FR-033). */
    worktree_count?: number
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

export type ProjectMode = 'plan' | 'build'

export type WorktreeSummary = {
    id: string
    name: string
    branch: string
    available: boolean
}

export type WorktreeDetail = WorktreeSummary & {
    base_ref: string
    path: string
    created_at: number
    changed: number
    untracked: number
    branch_merged: boolean
}

export type CreateWorktree = {
    worktree: WorktreeSummary
    topic: string
    channel_id: string
}

export type FileSuggestion = { path: string; kind: 'file' | 'dir' }

export type GitSummary = {
    available: boolean
    branch: string | null
    detached: boolean
    ahead: number
    behind: number
    changed: number
}

export type GitChange = {
    path: string
    old_path: string | null
    kind: 'modified' | 'added' | 'deleted' | 'renamed' | 'untracked' | 'conflicted'
    added: number
    removed: number
}

export type GitFileDiff = {
    path: string
    diff: string
    truncated: boolean
    binary: boolean
}

export type GitBranch = {
    name: string
    current: boolean
    checked_out_in: { worktree_id: string | null; name: string } | null
}

/** Switch one chat between Plan and Build; effective from the next turn. */
export const setProjectMode = (
    agentId: string,
    projectId: string,
    topic: string,
    mode: ProjectMode
) =>
    api.put<{ mode: ProjectMode }>(
        `${base(agentId)}/${encodeURIComponent(projectId)}/sessions/${encodeURIComponent(topic)}/mode`,
        { mode }
    )

export const suggestProjectFiles = (
    agentId: string,
    projectId: string,
    q: string,
    topic?: string,
    limit = 20
) => {
    const params = new URLSearchParams({ q, limit: String(limit) })
    if (topic) params.set('topic', topic)
    return api.get<FileSuggestion[]>(
        `${base(agentId)}/${encodeURIComponent(projectId)}/files?${params}`
    )
}

export type ProjectSkill = {
    name: string
    description: string
    scope: string
    user_invocable: boolean
    argument_hint: string | null
    folder: string
}

/** The skills this project chat's folder carries, for the composer's `/` menu (spec 041 FR-035). */
export const listProjectSkills = (agentId: string, projectId: string, topic?: string) => {
    const suffix = topic ? `?topic=${encodeURIComponent(topic)}` : ''
    return api.get<ProjectSkill[]>(
        `${base(agentId)}/${encodeURIComponent(projectId)}/skills${suffix}`
    )
}

const gitBase = (agentId: string, projectId: string) =>
    `${base(agentId)}/${encodeURIComponent(projectId)}/git`

const gitSuffix = (topic?: string) => (topic ? `?topic=${encodeURIComponent(topic)}` : '')

export const getGitSummary = (agentId: string, projectId: string, topic?: string) =>
    api.get<GitSummary>(`${gitBase(agentId, projectId)}${gitSuffix(topic)}`)

export const getGitChanges = (agentId: string, projectId: string, topic?: string) =>
    api.get<GitChange[]>(`${gitBase(agentId, projectId)}/changes${gitSuffix(topic)}`)

export const getGitDiff = (
    agentId: string,
    projectId: string,
    path: string,
    topic?: string
) => {
    const params = new URLSearchParams({ path })
    if (topic) params.set('topic', topic)
    return api.get<GitFileDiff>(`${gitBase(agentId, projectId)}/diff?${params}`)
}

export const getGitBranches = (agentId: string, projectId: string, topic?: string) =>
    api.get<GitBranch[]>(`${gitBase(agentId, projectId)}/branches${gitSuffix(topic)}`)

export const switchBranch = (
    agentId: string,
    projectId: string,
    body: { branch: string; create?: boolean; topic?: string }
) => api.post<GitSummary>(`${gitBase(agentId, projectId)}/switch`, body)

const worktreeBase = (agentId: string, projectId: string) =>
    `${base(agentId)}/${encodeURIComponent(projectId)}/worktrees`

export const listWorktrees = (agentId: string, projectId: string) =>
    api.get<WorktreeSummary[]>(worktreeBase(agentId, projectId))

export const createWorktree = (
    agentId: string,
    projectId: string,
    body: { name: string; base: string; branch?: string }
) => api.post<CreateWorktree>(worktreeBase(agentId, projectId), body)

export const getWorktree = (agentId: string, projectId: string, id: string) =>
    api.get<WorktreeDetail>(`${worktreeBase(agentId, projectId)}/${encodeURIComponent(id)}`)

export const deleteWorktree = (
    agentId: string,
    projectId: string,
    id: string,
    options: { force?: boolean; deleteBranch?: boolean } = {}
) => {
    const params = new URLSearchParams({
        force: String(!!options.force),
        delete_branch: String(!!options.deleteBranch),
    })
    return api.delete<{ deleted: boolean; branch_deleted: boolean }>(
        `${worktreeBase(agentId, projectId)}/${encodeURIComponent(id)}?${params}`
    )
}
