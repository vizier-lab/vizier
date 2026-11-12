import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { HIDDEN_TITLE, MySharesPage, ShareRow } from './MySharesPage'
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
    getShare: vi.fn(),
    sharePath: (id: string) => `/share/${encodeURIComponent(id)}`,
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
        expect(html).not.toContain('All shares')

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
        expect(html).toContain('All shares')
    })
})

describe('ShareRow', () => {
    const base = {
        share_id: 's1',
        title: 'Trip planning',
        agent_id: 'a1',
        agent_name: 'Atlas',
        topic_id: 't1',
        audience: 'public' as const,
        people_count: 0,
        include_trail: false,
        size_bytes: 1,
        created_at: 1790000000000,
        revoked_at: null,
        removals: { credentials: 0, paths: 0, recall: 0, reasoning: 0, media: 0 },
    }
    const noop = () => undefined
    const row = (item: Parameters<typeof ShareRow>[0]['item'], all = false) =>
        renderToString(
            <table>
                <tbody>
                    <ShareRow item={item} all={all} onCopy={noop} onDownload={noop} onRevoke={noop} />
                </tbody>
            </table>
        )

    it('an own share has Open (to /share/<id>), Copy link, Download file and Revoke', () => {
        const html = row(base)
        expect(html).toContain('href="/share/s1"')
        expect(html).toContain('Open')
        expect(html).toContain('Copy link')
        expect(html).toContain('Download file')
        expect(html).toContain('Revoke')
        expect(html).toContain('rel="noopener noreferrer"')
    })

    it('an admin-list item without a title shows no title and cannot be opened, copied or downloaded, only revoked', () => {
        const { title: _omit, ...hidden } = base
        const html = row({ ...hidden, creator_name: 'bob', audience: 'specific', people_count: 3 }, true)
        expect(html).toContain(HIDDEN_TITLE)
        expect(html).not.toContain('undefined')
        expect(html).not.toContain('href="/share/')
        expect(html).not.toContain('Copy link')
        expect(html).not.toContain('Download file')
        expect(html).toContain('Revoke')
        expect(html).toContain('3 people')
        expect(html).toContain('bob')
    })

    it('an admin sees Open for a share they are in the audience of, but no download (author only)', () => {
        const html = row({ ...base, creator_name: 'bob' }, true)
        expect(html).toContain('href="/share/s1"')
        expect(html).not.toContain('Download file')
    })

    it('a revoked share offers no actions', () => {
        const html = row({ ...base, revoked_at: 1790000100000 })
        expect(html).toContain('revoked')
        expect(html).not.toContain('Open')
        expect(html).not.toContain('Revoke</')
    })
})
