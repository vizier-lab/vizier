import { create } from 'zustand'
import {
    getGitSummary,
    getProjectLsp,
    listProjectSessions,
    listProjects,
    listWorktrees,
    setProjectMode as setProjectModeApi,
    type GitSummary,
    type LspSnapshot,
    type LspView,
    type Project,
    type ProjectMode,
    type ProjectSessionRow,
    type WorktreeSummary,
} from '../api/endpoints/projects'

const REFRESH_MS = 15_000

export const projectKey = (agentId: string, projectId: string) => `${agentId}/${projectId}`

/** Projects whose language-server panel is on screen, with how many watchers each has. */
const watched = new Map<string, number>()

type ProjectsStore = {
    /** Per agent, newest first. Kept on a failed poll rather than blanked. */
    byAgent: Record<string, Project[]>
    loaded: Record<string, boolean>
    /** The caller's own sessions per `projectKey`. */
    sessions: Record<string, ProjectSessionRow[]>
    refresh: (agentId: string) => Promise<void>
    /** Re-reads one project's sessions; safe to call repeatedly. */
    loadSessions: (agentId: string, projectId: string) => Promise<void>
    /** A session this client just created: it is listed before the server's next poll. */
    insertSession: (agentId: string, projectId: string, session: ProjectSessionRow) => void
    /** The project's worktrees, per `projectKey`. */
    worktrees: Record<string, WorktreeSummary[]>
    /** The git header per `projectKey/topic`. */
    git: Record<string, GitSummary>
    /** Switch one chat between Plan and Build, optimistically. */
    setMode: (agentId: string, projectId: string, topic: string, mode: ProjectMode) => Promise<void>
    refreshWorktrees: (agentId: string, projectId: string) => Promise<void>
    refreshGit: (agentId: string, projectId: string, topic: string) => Promise<void>
    /** The project is gone: drop it and its sessions. */
    forget: (agentId: string, projectId: string) => void
    /** Language servers per project id (spec 054), kept live by the inbox stream. */
    lsp: Record<string, LspView>
    loadLsp: (agentId: string, projectId: string) => Promise<void>
    /** A pushed snapshot: it replaces the status and keeps what only a read carries. */
    applyLsp: (snapshot: LspSnapshot) => void
    /** The panel is open on this project: keep it read, and re-read it after a gap. */
    watchLsp: (agentId: string, projectId: string) => () => void
    /** Re-read every watched project: the stream said it may have missed something. */
    reloadWatchedLsp: () => void
    /** The panel's own change (switch, override): take the server's answer as it is. */
    setLsp: (projectId: string, view: LspView) => void
    /** Polls while the tab is visible; returns a stop function. */
    start: (agentId: string) => () => void
}

function mergeSession(list: ProjectSessionRow[], session: ProjectSessionRow): ProjectSessionRow[] {
    return [session, ...list.filter((s) => s.topic !== session.topic)]
}

/** One shared empty list: a selector that returns a fresh `[]` makes React see a new snapshot every call. */
export const NO_PROJECTS: Project[] = []

export const useProjects = create<ProjectsStore>()((set, get) => ({
    byAgent: {},
    loaded: {},
    sessions: {},
    worktrees: {},
    git: {},
    lsp: {},

    refresh: async (agentId) => {
        try {
            const projects = (await listProjects(agentId)) ?? []
            set((cur) => ({
                byAgent: { ...cur.byAgent, [agentId]: projects },
                loaded: { ...cur.loaded, [agentId]: true },
            }))
        } catch {
            /* keep the previous list: a failed poll is not an empty one */
        }
    },

    loadSessions: async (agentId, projectId) => {
        const key = projectKey(agentId, projectId)
        try {
            const rows = (await listProjectSessions(agentId, projectId)) ?? []
            set((cur) => ({ sessions: { ...cur.sessions, [key]: rows } }))
        } catch {
            /* keep the previous sessions */
        }
    },

    insertSession: (agentId, projectId, session) => {
        const key = projectKey(agentId, projectId)
        set((cur) => {
            const projects = cur.byAgent[agentId] ?? []
            const known = cur.sessions[key] ?? []
            const isNew = !known.some((s) => s.topic === session.topic)
            return {
                sessions: {
                    ...cur.sessions,
                    [key]: mergeSession(known, session),
                },
                byAgent: {
                    ...cur.byAgent,
                    [agentId]: projects.map((p) =>
                        p.id === projectId && isNew
                            ? { ...p, session_count: p.session_count + 1 }
                            : p
                    ),
                },
            }
        })
    },

    setMode: async (agentId, projectId, topic, mode) => {
        const key = projectKey(agentId, projectId)
        const previous = (get().sessions[key] ?? []).find((s) => s.topic === topic)?.mode
        set((cur) => ({
            sessions: {
                ...cur.sessions,
                [key]: (cur.sessions[key] ?? []).map((s) =>
                    s.topic === topic ? { ...s, mode } : s
                ),
            },
        }))
        try {
            await setProjectModeApi(agentId, projectId, topic, mode)
        } catch (error) {
            // Revert: the server refused, so the chat still has its old mode.
            set((cur) => ({
                sessions: {
                    ...cur.sessions,
                    [key]: (cur.sessions[key] ?? []).map((s) =>
                        s.topic === topic ? { ...s, mode: previous ?? 'plan' } : s
                    ),
                },
            }))
            throw error
        }
    },

    refreshWorktrees: async (agentId, projectId) => {
        const key = projectKey(agentId, projectId)
        try {
            const worktrees = (await listWorktrees(agentId, projectId)) ?? []
            set((cur) => ({ worktrees: { ...cur.worktrees, [key]: worktrees } }))
        } catch {
            /* keep the previous list */
        }
    },

    refreshGit: async (agentId, projectId, topic) => {
        const key = `${projectKey(agentId, projectId)}/${topic}`
        try {
            const summary = await getGitSummary(agentId, projectId, topic)
            set((cur) => ({ git: { ...cur.git, [key]: summary } }))
        } catch {
            /* a failed read leaves the header as it was */
        }
    },

    loadLsp: async (agentId, projectId) => {
        try {
            const view = await getProjectLsp(agentId, projectId)
            if (view) set((cur) => ({ lsp: { ...cur.lsp, [projectId]: view } }))
        } catch {
            /* keep what is held: a failed read is not an empty list */
        }
    },

    applyLsp: (snapshot) =>
        set((cur) => {
            const held = cur.lsp[snapshot.project_id]
            // A project nobody read yet still shows its status; the next read adds the rest.
            return {
                lsp: {
                    ...cur.lsp,
                    [snapshot.project_id]: { ...held, ...snapshot, can_manage: held?.can_manage ?? false },
                },
            }
        }),

    setLsp: (projectId, view) => set((cur) => ({ lsp: { ...cur.lsp, [projectId]: view } })),

    watchLsp: (agentId, projectId) => {
        const key = projectKey(agentId, projectId)
        watched.set(key, (watched.get(key) ?? 0) + 1)
        void get().loadLsp(agentId, projectId)
        return () => {
            const left = (watched.get(key) ?? 1) - 1
            if (left <= 0) watched.delete(key)
            else watched.set(key, left)
        }
    },

    reloadWatchedLsp: () => {
        for (const key of watched.keys()) {
            const [agentId, projectId] = key.split('/')
            void get().loadLsp(agentId, projectId)
        }
    },

    forget: (agentId, projectId) => {
        const key = projectKey(agentId, projectId)
        set((cur) => {
            const { [key]: _sessions, ...sessions } = cur.sessions
            const { [key]: _worktrees, ...worktrees } = cur.worktrees
            return {
                sessions,
                worktrees,
                byAgent: {
                    ...cur.byAgent,
                    [agentId]: (cur.byAgent[agentId] ?? []).filter((p) => p.id !== projectId),
                },
            }
        })
    },

    start: (agentId) => {
        const tick = () => {
            if (typeof document !== 'undefined' && document.visibilityState !== 'visible') return
            void get().refresh(agentId)
            for (const project of get().byAgent[agentId] ?? []) {
                void get().loadSessions(agentId, project.id)
                void get().refreshWorktrees(agentId, project.id)
            }
        }
        const timer = setInterval(tick, REFRESH_MS)
        return () => clearInterval(timer)
    },
}))
