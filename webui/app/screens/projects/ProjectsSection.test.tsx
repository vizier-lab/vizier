import { renderToString } from 'react-dom/server'
import { MemoryRouter } from 'react-router'
import { beforeEach, describe, expect, it } from 'vitest'
import { useSession } from '../../stores/session'
import { ProjectRow } from './ProjectRow'
import { projectsSectionVisible } from './ProjectsSection'

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

beforeEach(() => {
    useSession.setState({ user: null })
})

describe('the Projects section', () => {
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
        const html = renderToString(
            <MemoryRouter>
                <ProjectRow
                    agentId="dp-a"
                    project={project}
                    sessions={[
                        { topic: 't1', title: 'Investigate the parser', is_thinking: false, last_activity: 1 },
                    ]}
                    expanded
                    onToggle={() => {}}
                />
            </MemoryRouter>
        )
        expect(html).toContain('alpha')
        expect(html).toContain('Investigate the parser')

        const empty = renderToString(
            <MemoryRouter>
                <ProjectRow
                    agentId="dp-a"
                    project={{ ...project, session_count: 0 }}
                    sessions={[]}
                    expanded
                    onToggle={() => {}}
                />
            </MemoryRouter>
        )
        expect(empty).toContain('No sessions yet')
    })
})
