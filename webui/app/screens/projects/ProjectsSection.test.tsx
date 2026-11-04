import { renderToString } from 'react-dom/server'
import { MemoryRouter } from 'react-router'
import { beforeEach, describe, expect, it } from 'vitest'
import type { ProjectSessionRow } from '../../api/endpoints/projects'
import { NO_PROJECTS, useProjects } from '../../stores/projects'
import { useSession } from '../../stores/session'
import { ProjectRow } from './ProjectRow'
import { projectsSectionVisible, shouldOfferOpenProject } from './ProjectsSection'

const person = (permissions: string[]) => ({
    user_id: 'user-1',
    username: 'alice',
    role_name: 'member',
    permissions,
})

const project = {
    id: 'p3xY9aQ2mZ',
    agent_id: 'dp-a',
    name: 'alpha',
    folder: '/tmp/work/alpha',
    available: true,
    shell_enabled: true,
    owner_id: 'user-1',
    is_owner: true,
    session_count: 1,
    created_at: 0,
}

const session = (topic: string, title: string, at: number | null = 1): ProjectSessionRow => ({
    topic,
    title,
    is_thinking: false,
    last_activity: at,
})

const at = (topic: string, title: string, worktree?: ProjectSessionRow['worktree']): ProjectSessionRow => ({
    ...session(topic, title),
    worktree: worktree ?? null,
})

const render = (
    sessions: ProjectSessionRow[],
    { row = project, path = '/' }: { row?: typeof project; path?: string } = {}
) =>
    renderToString(
        <MemoryRouter initialEntries={[path]}>
            <ProjectRow agentId="dp-a" project={row} sessions={sessions} />
        </MemoryRouter>
    )

beforeEach(() => {
    useSession.setState({ user: null })
})

describe('the Projects section', () => {
    it('offers Open a Project for every empty ordinary chat, including after projects exist', () => {
        expect(shouldOfferOpenProject(undefined)).toBe(true)
        expect(shouldOfferOpenProject('project-1')).toBe(false)
    })

    it('is shown only to a permitted viewer of an agent with projects', () => {
        useSession.setState({ user: person(['all_agents:projects']) })
        expect(projectsSectionVisible({ owner_id: 'user-2' }, [project])).toBe(true)
        expect(projectsSectionVisible({ owner_id: 'user-2' }, [])).toBe(false)
        useSession.setState({ user: person(['all_agents:view']) })
        expect(projectsSectionVisible({ owner_id: 'user-2' }, [project])).toBe(false)
        useSession.setState({ user: person(['owned_agents:projects']) })
        expect(projectsSectionVisible({ owner_id: 'user-1' }, [project])).toBe(true)
        expect(projectsSectionVisible({ owner_id: 'user-2' }, [project])).toBe(false)
    })

    it('renders a project and the caller’s sessions, with the empty-project state', () => {
        useSession.setState({ user: person(['all_agents:projects']) })
        const html = render([session('t1', 'Investigate the parser')])
        expect(html).toContain('alpha')
        expect(html).toContain('Investigate the parser')

        expect(render([], { row: { ...project, session_count: 0 } })).toContain('No sessions yet')
    })

    it('lists the sessions flat, in the Today row style, with no indent', () => {
        const html = render([session('t1', 'Investigate the parser')])
        // The row is the Sessions list's: one 30px line at the group's own left edge.
        expect(html).toContain('h-30')
        expect(html).toContain('rounded-lg px-10')
        expect(html).toContain('type-ui')
        // The old indented block under the chevron is gone, and nothing collapses.
        expect(html).not.toContain('pl-24')
        expect(html).not.toContain('Expand alpha')
        expect(html).not.toContain('Collapse alpha')
    })

    it('marks the open session the way the Sessions list does', () => {
        const html = render([session('t1', 'A'), session('t2', 'B')], {
            path: '/dp-a/projects/p3xY9aQ2mZ/chat/t2',
        })
        expect(html).toContain('aria-current="page"')
        expect(html).toContain('bg-surface text-fg')
        expect(html).toContain('type-ui-medium')
    })

    it('shows five sessions and offers Show all for the rest', () => {
        const many = ['t1', 't2', 't3', 't4', 't5', 't6', 't7'].map((t) => session(t, `Session ${t}`))
        const html = render(many)
        for (const shown of ['Session t1', 'Session t5']) expect(html).toContain(shown)
        for (const hidden of ['Session t6', 'Session t7']) expect(html).not.toContain(hidden)
        expect(html).toContain('Show all')
        expect(html).toContain('aria-expanded="false"')
    })

    it('shows a worktree chat first and never hides it, even when it is the oldest', () => {
        // The server lists by creation, so an old worktree chat arrives last; it is still first here.
        const html = render([
            ...['t1', 't2', 't3', 't4', 't5', 't6'].map((t) => session(t, `Session ${t}`)),
            at('w1', 'fix-ps', { id: 'w1', name: 'fix-ps', branch: 'fix-ps', available: true }),
        ])
        expect(html).toContain('fix-ps')
        expect(html.indexOf('fix-ps')).toBeLessThan(html.indexOf('Session t1'))
        // Five of the six plain sessions, plus the worktree chat.
        expect(html).toContain('Session t5')
        expect(html).not.toContain('Session t6')
    })

    it('offers no toggle when there is nothing hidden', () => {
        expect(render(['t1', 't2', 't3'].map((t) => session(t, `Session ${t}`)))).not.toContain('Show all')
        expect(render([session('t1', 'Only one')])).not.toContain('Show less')
        // Five is the preview itself, not a cut: no toggle until a sixth session exists.
        const five = ['t1', 't2', 't3', 't4', 't5'].map((t) => session(t, `Session ${t}`))
        expect(render(five)).not.toContain('Show all')
        expect(render([...five, session('t6', 'Session t6')])).toContain('Show all')
    })

    it('gives an agent whose projects have not loaded one stable empty list', () => {
        // A selector that returned a fresh [] each call made React see a new snapshot every
        // time and loop until the app went blank.
        const select = (state: ReturnType<typeof useProjects.getState>) =>
            state.byAgent['not-loaded'] ?? NO_PROJECTS
        const state = useProjects.getState()
        expect(select(state)).toBe(select(state))
    })

    it('marks a worktree chat with its branch once, not twice', () => {
        useSession.setState({ user: person(['all_agents:projects']) })
        const html = render([
            at('t2', 'fix-ps', { id: 'w1', name: 'fix-ps', branch: 'fix-ps', available: true }),
            at('t3', 'Try the parser', { id: 'w2', name: 'try', branch: 'try-parser', available: true }),
        ])
        // Same name as its branch: the title says it. A different branch is shown beside the title.
        expect(html.match(/>fix-ps</g)?.length).toBe(1)
        expect(html).toContain('>try-parser<')
    })
})
