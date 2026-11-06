import { api, queryString } from '../client'
import type { ChatMessage } from '../frames'

// Workspaces (specs/053-workspace-channel/contracts/http-api.md). A person with no standing in a
// Workspace gets 404 from every route, the same as for one that does not exist.

export type WorkspaceRole = 'owner' | 'member' | 'admin'

export type TopicSummary = { id: string; name: string; unread: number }

export type WorkspaceSummary = {
    id: string
    name: string
    role: WorkspaceRole
    unread: number
    /** An agent is working in it. */
    working: boolean
    topics: TopicSummary[]
}

export type WorkspaceMember = {
    kind: 'person' | 'agent'
    handle: string
    name: string
    role: 'owner' | 'member'
    /** An agent deleted after it was added reads false. */
    available: boolean
}

export type WorkspaceDetail = WorkspaceSummary & { members: WorkspaceMember[] }

export type MentionState = 'pending' | 'woken' | 'unavailable' | 'dropped' | 'resolved'

export type EntryMention = { kind: 'person' | 'agent'; handle: string; state: MentionState }

export type EntryAuthor = { kind: 'person' | 'agent'; id: string; name: string; available: boolean }

export type EntryOrigin = { kind: 'task' | 'chat' | 'discord' | 'telegram' | 'workspace'; label: string }

export type WorkspaceEntry = {
    seq: number
    kind: 'person' | 'agent' | 'note'
    author: EntryAuthor
    body: string
    /** Unix milliseconds. */
    at: number
    mentions: EntryMention[]
    origin: EntryOrigin | null
    is_final: boolean
    activated_by_seq: number | null
    history_uid: string | null
    files: { file_id: string; name: string; mime_type: string; size: number }[]
}

export type WorkspaceStep = { agent_id: string; entry: ChatMessage }

export type Timeline = {
    entries: WorkspaceEntry[]
    steps: WorkspaceStep[]
    /** Pass as `before` for the page above; null when nothing is older. */
    next_before: number | null
}

export type PostResult = { entry: WorkspaceEntry | null; note: WorkspaceEntry | null }

export type CommandsMenu = {
    commands: { name: string; description: string }[]
    skills: { name: string; description: string; agents: string[] }[]
}

const base = (id: string) => `/workspaces/${encodeURIComponent(id)}`
const topicBase = (id: string, topic: string) => `${base(id)}/topics/${encodeURIComponent(topic)}`

export const listWorkspaces = () => api.get<WorkspaceSummary[]>('/workspaces')
export const getWorkspace = (id: string) => api.get<WorkspaceDetail>(base(id))
export const createWorkspace = (name: string, firstTopic?: string) =>
    api.post<WorkspaceDetail>('/workspaces', { name, first_topic: firstTopic })
export const createTopic = (id: string, name: string) => api.post<TopicSummary>(`${base(id)}/topics`, { name })
export const addMember = (id: string, kind: 'person' | 'agent', handle: string, role?: 'owner' | 'member') =>
    api.post<WorkspaceMember>(`${base(id)}/members`, { kind, handle, role })
export const removeMember = (id: string, kind: 'person' | 'agent', handle: string) =>
    api.delete<void>(`${base(id)}/members/${kind}/${encodeURIComponent(handle)}`)

export const getTimeline = (id: string, topic: string, before?: number | null, limit?: number) =>
    api.get<Timeline>(`${topicBase(id, topic)}/entries${queryString({ before: before ?? undefined, limit })}`)
export const postEntry = (id: string, topic: string, text: string, files: string[] = []) =>
    api.post<PostResult>(`${topicBase(id, topic)}/entries`, { text, files })
export const markTopicRead = (id: string, topic: string) => api.post<{ marked: number }>(`${topicBase(id, topic)}/read`)
export const getCommands = (id: string) => api.get<CommandsMenu>(`${base(id)}/commands`)
export const stopAll = (id: string) => api.post<{ stopped: number; dropped: number }>(`${base(id)}/stop`)
export const stopAgent = (id: string, topic: string, agent: string) =>
    api.post<{ stopped: number; dropped: number }>(`${topicBase(id, topic)}/turns/${encodeURIComponent(agent)}/stop`)

/** The wire frames a Workspace topic socket pushes (contracts/socket-frames.md). */
export type WorkspaceFrame =
    | { hello: { latest_seq: number; working: { agent: string; phase: string }[] } }
    | { workspace_entry: WorkspaceEntry }
    | { history_entry: { agent_id: string; entry: ChatMessage } }
    | { workspace_mentions: { seq: number; mentions: EntryMention[] } }
    | { agent_state: { agent: string; phase: 'thinking' | 'idle' } }
    | { member_changed: Record<string, never> }
    | { closed: { reason: 'removed' | 'deleted' | 'token_expired' } }
    | { resync: Record<string, never> }
    | { post_error: { status: number; code: string | null; message: string } }
