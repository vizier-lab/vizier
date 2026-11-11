import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { MySharesPage } from './MySharesPage'
import { useSession } from '../../stores/session'

vi.mock('../../api/shares', () => ({
    listShares: vi.fn(async () => [
        {
            share_id: 's1',
            title: 'Trip planning',
            agent_id: 'a1',
            agent_name: 'Atlas',
            topic_id: 't1',
            audience: 'public',
            people_count: 0,
            include_trail: false,
            size_bytes: 1024,
            created_at: 1790000000000,
            revoked_at: null,
            removals: {},
        },
        {
            share_id: 's2',
            title: 'Revoked share',
            agent_id: 'a1',
            agent_name: 'Atlas',
            topic_id: 't1',
            audience: 'specific',
            people_count: 2,
            include_trail: true,
            size_bytes: 2048,
            created_at: 1780000000000,
            revoked_at: 1780000100000,
            removals: {},
        },
    ]),
    revokeShare: vi.fn(async () => {}),
}))

describe('MySharesPage', () => {
    it('renders the page header and description', () => {
        useSession.setState({
            user: {
                user_id: 'u1',
                username: 'alice',
                role_name: 'member',
                permissions: ['owned_agents:share_conversations'],
            },
        })
        const html = renderToString(<MySharesPage />)
        expect(html).toContain('Shared conversations')
        expect(html).toContain('Manage links created to share conversations')
    })

    it('shows admin all-shares toggle only for users with all_agents permission', () => {
        // Non-admin
        useSession.setState({
            user: {
                user_id: 'u1',
                username: 'alice',
                role_name: 'member',
                permissions: ['owned_agents:share_conversations'],
            },
        })
        let html = renderToString(<MySharesPage />)
        expect(html).not.toContain('All instance shares')

        // Admin
        useSession.setState({
            user: {
                user_id: 'root',
                username: 'root',
                role_name: 'system',
                permissions: ['all_agents:share_conversations'],
            },
        })
        html = renderToString(<MySharesPage />)
        expect(html).toContain('All instance shares')
    })
})
