import { renderToString } from 'react-dom/server'
import { StaticRouter } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '../../api/client'
import { buildPreview } from '../../lib/share/preview'
import { serializeEntries, ShareError } from '../../lib/share/serialize'
import { loadConversation, pngDeps } from '../../lib/share/testing'
import { useSession } from '../../stores/session'

const create = vi.hoisted(() => vi.fn(async () => ({ share_id: 'abc_DEF-123', url_path: '/share/abc_DEF-123', created_at: 1 })))
vi.mock('../../api/shares', async (orig) => ({ ...(await orig<typeof import('../../api/shares')>()), createShare: create }))

import { COPY_WARNING, createFailure, createLink, DESKTOP_NOTICE, leftOutLines, ShareDialog, summaryLine, TRAIL_HELP, warningLines } from './ShareDialog'
import { ConversationHeader, EMPTY_SHARE_REASON } from './ConversationHeader'

async function previewOf(includeTrail = false) {
    const fx = loadConversation()
    const result = await serializeEntries(fx.entries, { agentName: fx.agent_name, title: 'T', includeTrail, files: fx.files, media: pngDeps(fx.pngBase64) })
    return { result, preview: buildPreview(result) }
}

beforeEach(() => create.mockClear())

describe('the dialog text', () => {
    it('counts what is shared and what is left out, trail off by default', async () => {
        const { preview } = await previewOf(false)
        expect(summaryLine(preview)).toMatch(/^\d+ messages? · .*of 5\.0 MiB|KiB of/)
        const out = leftOutLines(preview).join(' | ')
        expect(out).toMatch(/\d/)
        expect(out).toMatch(/memory passage|reasoning step|activity step/)
        expect(preview.includeTrail).toBe(false)
        expect(TRAIL_HELP).toMatch(/^Off:/)
        expect(COPY_WARNING).toMatch(/cannot be taken back/)
        expect(Array.isArray(warningLines(preview))).toBe(true)
    })

    it('shows the trail count instead of "left out" when the trail is on', async () => {
        const { preview } = await previewOf(true)
        expect(summaryLine(preview)).toMatch(/activity step/)
        expect(leftOutLines(preview).join(' ')).not.toMatch(/activity step \(tool call/)
    })

    it('words a refusal, a limit and a server failure', () => {
        expect(createFailure(new ShareError('too_large', 'Too big'))).toBe('Too big')
        expect(createFailure(new ApiError(429, 'rate_limited', 'x'))).toMatch(/Too many/)
        expect(createFailure(new ApiError(403, 'forbidden', 'x'))).toMatch(/not allowed/)
        expect(createFailure(new ApiError(503, 'unavailable', 'Service Unavailable'))).toMatch(/Nothing was shared/)
        expect(createFailure(new ApiError(413, 'too_large', 'x'))).toMatch(/too large/)
        expect(createFailure('weird')).toMatch(/could not be created/)
    })
})

describe('creating the link', () => {
    it('sends one public request carrying exactly what was previewed, and returns the link', async () => {
        const { result } = await previewOf(false)
        const link = await createLink({ agentId: 'a1', topicId: 't1', title: 'T' }, result, 'https://vizier.example')
        expect(link).toBe('https://vizier.example/share/abc_DEF-123')
        expect(create).toHaveBeenCalledTimes(1)
        const req = (create.mock.calls[0] as unknown[])[0] as { audience: string; include_trail: boolean; snapshot: unknown; removals: unknown }
        expect(req.audience).toBe('public')
        expect(req.include_trail).toBe(false)
        expect(req.snapshot).toBe(result.snapshot)
        expect(req.removals).toEqual(result.removals)
    })

    it('sends signed_in and specific requests with appropriate audience and people', async () => {
        const { result } = await previewOf(false)
        await createLink({ agentId: 'a1', topicId: 't1', title: 'T', audience: 'signed_in' }, result, 'https://vizier.example')
        expect(create).toHaveBeenLastCalledWith(expect.objectContaining({ audience: 'signed_in' }))

        await createLink(
            { agentId: 'a1', topicId: 't1', title: 'T', audience: 'specific', people: ['u-bob'] },
            result,
            'https://vizier.example'
        )
        expect(create).toHaveBeenLastCalledWith(expect.objectContaining({ audience: 'specific', people: ['u-bob'] }))
    })

    it('renders all three audience choices when opened', () => {
        const html = renderToString(<ShareDialog open={true} onClose={() => undefined} agentId="a" topicId="t" title="T" agentName="A" />)
        expect(html).toContain('Specific people')
        expect(html).toContain('Anyone signed in on this instance')
        expect(html).toContain('Anyone with the link (public)')
    })

    it('a closed dialog (cancel) reads and stores nothing', () => {
        const html = renderToString(<ShareDialog open={false} onClose={() => undefined} agentId="a" topicId="t" title="T" agentName="A" />)
        expect(html).not.toContain('Create link')
        expect(create).not.toHaveBeenCalled()
    })
})

describe('the Share button', () => {
    const person = (permissions: string[]) => ({ user_id: 'u1', username: 'alice', role_name: 'm', permissions })
    const head = (share: Parameters<typeof ConversationHeader>[0]['share']) =>
        renderToString(
            <StaticRouter location="/a/chat/t">
                <ConversationHeader agentId="a" title="T" meta="m" share={share} />
            </StaticRouter>
        )

    it('is hidden without the permission', () => {
        useSession.setState({ user: person([]) })
        expect(head({ topicId: 't', agent: { owner_id: 'u1' }, agentName: 'A', empty: false })).not.toContain('Share this conversation')
    })
    it('is hidden on a conversation that cannot be shared', () => {
        useSession.setState({ user: person(['all_agents:share_conversations']) })
        expect(head(undefined)).not.toContain('Share this conversation')
    })
    it('is shown and enabled with the permission', () => {
        useSession.setState({ user: person(['owned_agents:share_conversations']) })
        const html = head({ topicId: 't', agent: { owner_id: 'u1' }, agentName: 'A', empty: false })
        expect(html).toContain('aria-label="Share this conversation"')
        expect(html).not.toMatch(/aria-label="Share this conversation"[^>]*disabled/)
    })
    it('is disabled with the reason on an empty conversation', () => {
        useSession.setState({ user: person(['owned_agents:share_conversations']) })
        const html = head({ topicId: 't', agent: { owner_id: 'u1' }, agentName: 'A', empty: true })
        expect(html).toContain(EMPTY_SHARE_REASON)
        expect(html).toMatch(/<button[^>]*disabled[^>]*>|<button[^>]*aria-label="Share this conversation\. Nothing[^>]*disabled/)
    })
})

it('has a desktop notice that points to the offline file', () => {
    expect(DESKTOP_NOTICE).toBe('A link needs a server others can reach. Use Offline file.')
})
