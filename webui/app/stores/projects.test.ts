import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('../api/endpoints/projects', async (importOriginal) => {
    const actual = await importOriginal<typeof import('../api/endpoints/projects')>()
    return {
        ...actual,
        listProjects: vi.fn(),
        listProjectSessions: vi.fn(),
        getProjectLsp: vi.fn(),
    }
})

import * as api from '../api/endpoints/projects'
import { projectKey, useProjects } from './projects'

const list = vi.mocked(api.listProjects)
const sessions = vi.mocked(api.listProjectSessions)
const readLsp = vi.mocked(api.getProjectLsp)

function project(id: string, sessionCount = 0): api.Project {
    return {
        id,
        agent_id: 'dp-a',
        name: id,
        folder: `/tmp/${id}`,
        available: true,
        shell_enabled: true,
        lsp_enabled: true,
        owner_id: 'u1',
        is_owner: true,
        session_count: sessionCount,
        created_at: 0,
    }
}

function session(topic: string): api.ProjectSessionRow {
    return { topic, title: 'New session', is_thinking: false, last_activity: null }
}

beforeEach(() => {
    vi.clearAllMocks()
    useProjects.setState({ byAgent: {}, loaded: {}, sessions: {}, lsp: {} })
})

describe('the projects store', () => {
    it('lists an agent’s projects', async () => {
        list.mockResolvedValue([project('alpha', 2)])
        await useProjects.getState().refresh('dp-a')
        expect(useProjects.getState().byAgent['dp-a']).toHaveLength(1)
        expect(useProjects.getState().loaded['dp-a']).toBe(true)
    })

    it('keeps the previous list when a poll fails', async () => {
        list.mockResolvedValueOnce([project('alpha')])
        await useProjects.getState().refresh('dp-a')
        list.mockRejectedValueOnce(new Error('offline'))
        await useProjects.getState().refresh('dp-a')
        expect(useProjects.getState().byAgent['dp-a']).toHaveLength(1)
    })

    it('loads only the caller’s sessions for a project', async () => {
        sessions.mockResolvedValue([session('t1')])
        await useProjects.getState().loadSessions('dp-a', 'alpha')
        expect(useProjects.getState().sessions[projectKey('dp-a', 'alpha')]).toHaveLength(1)
    })

    it('inserts a new session once and counts it', async () => {
        useProjects.setState({ byAgent: { 'dp-a': [project('alpha', 1)] } })
        useProjects.getState().insertSession('dp-a', 'alpha', session('t2'))
        useProjects.getState().insertSession('dp-a', 'alpha', session('t2'))
        const key = projectKey('dp-a', 'alpha')
        expect(useProjects.getState().sessions[key].map((s) => s.topic)).toEqual(['t2'])
        expect(useProjects.getState().byAgent['dp-a'][0].session_count).toBe(2)
    })

    it('forgets a removed project and its sessions', async () => {
        useProjects.setState({
            byAgent: { 'dp-a': [project('alpha'), project('beta')] },
            sessions: { [projectKey('dp-a', 'alpha')]: [session('t1')] },
        })
        useProjects.getState().forget('dp-a', 'alpha')
        expect(useProjects.getState().byAgent['dp-a'].map((p) => p.id)).toEqual(['beta'])
        expect(useProjects.getState().sessions[projectKey('dp-a', 'alpha')]).toBeUndefined()
    })
})

describe('language servers', () => {
    const snapshot = (over: Partial<api.LspSnapshot> = {}): api.LspSnapshot => ({
        project_id: 'alpha',
        enabled: true,
        capacity: { in_use: 0, max: 6 },
        servers: [],
        ...over,
    })
    const view = (over: Partial<api.LspView> = {}): api.LspView => ({ ...snapshot(), can_manage: true, languages: [], overrides: [], ...over })

    it('reads a project and keeps what only a read carries when a snapshot is pushed', async () => {
        readLsp.mockResolvedValue(view({ languages: [{ id: 'rust', name: 'Rust', default_command: 'rust-analyzer' }] }))
        await useProjects.getState().loadLsp('dp-a', 'alpha')
        useProjects.getState().applyLsp(snapshot({ capacity: { in_use: 2, max: 6 } }))
        const held = useProjects.getState().lsp.alpha
        expect(held.capacity.in_use).toBe(2)
        expect(held.can_manage).toBe(true)
        expect(held.languages).toHaveLength(1)
    })

    it('shows a pushed snapshot for a project nobody read yet, as view only', () => {
        useProjects.getState().applyLsp(snapshot({ project_id: 'beta' }))
        expect(useProjects.getState().lsp.beta.can_manage).toBe(false)
    })

    it('keeps the held status when a read fails', async () => {
        readLsp.mockResolvedValueOnce(view())
        await useProjects.getState().loadLsp('dp-a', 'alpha')
        readLsp.mockRejectedValueOnce(new Error('offline'))
        await useProjects.getState().loadLsp('dp-a', 'alpha')
        expect(useProjects.getState().lsp.alpha).toBeDefined()
    })

    it('re-reads watched projects after a gap, and stops when the last watcher leaves', async () => {
        readLsp.mockResolvedValue(view())
        const stopA = useProjects.getState().watchLsp('dp-a', 'alpha')
        const stopB = useProjects.getState().watchLsp('dp-a', 'alpha')
        await Promise.resolve()
        readLsp.mockClear()
        useProjects.getState().reloadWatchedLsp()
        expect(readLsp).toHaveBeenCalledTimes(1)
        stopA()
        readLsp.mockClear()
        useProjects.getState().reloadWatchedLsp()
        expect(readLsp).toHaveBeenCalledTimes(1)
        stopB()
        readLsp.mockClear()
        useProjects.getState().reloadWatchedLsp()
        expect(readLsp).not.toHaveBeenCalled()
    })
})
