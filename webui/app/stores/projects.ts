import { create } from 'zustand'
import {
    listProjectSessions,
    listProjects,
    type Project,
    type ProjectSessionRow,
} from '../api/endpoints/projects'

const REFRESH_MS = 15_000

export const projectKey = (agentId: string, projectId: string) => `${agentId}/${projectId}`

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
    /** The project is gone: drop it and its sessions. */
    forget: (agentId: string, projectId: string) => void
    /** Polls while the tab is visible; returns a stop function. */
    start: (agentId: string) => () => void
}

function mergeSession(list: ProjectSessionRow[], session: ProjectSessionRow): ProjectSessionRow[] {
    return [session, ...list.filter((s) => s.topic !== session.topic)]
}

export const useProjects = create<ProjectsStore>()((set, get) => ({
    byAgent: {},
    loaded: {},
    sessions: {},

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

    forget: (agentId, projectId) => {
        const key = projectKey(agentId, projectId)
        set((cur) => {
            const { [key]: _sessions, ...sessions } = cur.sessions
            return {
                sessions,
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
            }
        }
        const timer = setInterval(tick, REFRESH_MS)
        return () => clearInterval(timer)
    },
}))
