import { renderToString } from 'react-dom/server'
import { StaticRouter } from 'react-router'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '../../api/client'
import { buildPreview, formatBytes } from '../../lib/share/preview'
import { SHARE_LIMITS } from '../../lib/share/limits'
import { serializeEntries, ShareError } from '../../lib/share/serialize'
import { loadConversation, pngDeps } from '../../lib/share/testing'
import { useSession } from '../../stores/session'

const create = vi.hoisted(() => vi.fn(async () => ({ share_id: 'abc_DEF-123', url_path: '/share/abc_DEF-123', created_at: 1 })))
vi.mock('../../api/shares', async (orig) => ({ ...(await orig<typeof import('../../api/shares')>()), createShare: create }))

import { OFFLINE_WARNING, REVIEW_WARNING } from './ShareDialog'
import type { ShareListItem } from '../../api/shares'
import { AUTHOR_HELP, AUTHOR_LABEL, authorDefault, authorShown, CHOOSE_AUDIENCE, COPY_WARNING, createFailure, createLink, createShareFor, DESKTOP_NOTICE, leftOutLines, linkAudience, linkCreated, linkRows, LiveLinkList, MEDIA_HELP, NO_LINKS_YET, PUBLIC_CONFIRM, SECRET_REFUSED, ShareDialog, summaryLine, tabPreview, TRAIL_HELP, warningLines } from './ShareDialog'
import { ConversationHeader, EMPTY_SHARE_REASON } from './ConversationHeader'

async function previewOf(includeTrail = false, extra: Record<string, unknown> = {}) {
    const fx = loadConversation()
    const result = await serializeEntries(fx.entries, { agentName: fx.agent_name, title: 'T', includeTrail, files: fx.files, media: pngDeps(fx.pngBase64), ...extra })
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

    it('offers the switch that leaves pictures and audio out, off by default (FR-023)', () => {
        const html = renderToString(<ShareDialog open={true} onClose={() => undefined} agentId="a" topicId="t" title="T" agentName="A" />)
        expect(html).toContain('Leave out pictures and audio')
        expect(html).toContain(MEDIA_HELP)
        expect(MEDIA_HELP).toMatch(/smaller/)
        // Three switches (the trail, the pictures, the author's name), all off until the person turns one on
        // or the audience turns the name on for them (FR-007).
        expect(html.match(/role="switch"/g)).toHaveLength(3)
        expect(html.match(/aria-checked="false"/g)).toHaveLength(3)
    })

    it('counts the pictures the person left out, without calling them a warning', async () => {
        const { preview } = await previewOf(false, { skipMedia: true })
        expect(preview.images).toBe(0)
        expect(preview.removals.media).toBe(2)
        expect(summaryLine(preview)).not.toMatch(/image/)
        expect(leftOutLines(preview).join(' | ')).toContain('2 files left out')
        expect(warningLines(preview)).toEqual([])
    })

    it('words a refusal, a limit and a server failure', () => {
        expect(createFailure(new ShareError('too_large', 'Too big'))).toBe('Too big')
        expect(createFailure(new ApiError(429, 'rate_limited', 'x'))).toMatch(/Too many/)
        expect(createFailure(new ApiError(403, 'forbidden', 'x'))).toMatch(/not allowed/)
        expect(createFailure(new ApiError(503, 'unavailable', 'Service Unavailable'))).toMatch(/Nothing was shared/)
        expect(createFailure(new ApiError(422, 'secret_detected', 'The snapshot appears to contain a secret (GitHub token)'))).toBe(SECRET_REFUSED)
        expect(createFailure(new ApiError(422, 'secret_detected', 'x'))).not.toMatch(/GitHub/)
        expect(createFailure(new ApiError(422, 'trail_mismatch', 'Trail differs'))).toBe('Trail differs')
        expect(createFailure(new ApiError(413, 'too_large', 'x'))).toMatch(/too large/)
        expect(createFailure('weird')).toMatch(/could not be created/)
    })
})

describe('the author\'s name switch (FR-007)', () => {
    it('is on by default for the two instance audiences and off for a public link', () => {
        expect(authorDefault('link', 'specific')).toBe(true)
        expect(authorDefault('link', 'signed_in')).toBe(true)
        expect(authorDefault('link', 'public')).toBe(false)
    })
    it('is off for an offline file whatever the audience, and while no audience is chosen', () => {
        for (const audience of ['specific', 'signed_in', 'public'] as const) expect(authorDefault('offline', audience)).toBe(false)
        expect(authorDefault('offline', null)).toBe(false)
        expect(authorDefault('link', null)).toBe(false)
    })
    it('follows the audience until the person touches it, then their choice wins', () => {
        // untouched: the audience decides, and a change of audience changes the answer
        expect(authorShown(null, 'link', 'specific')).toBe(true)
        expect(authorShown(null, 'link', 'public')).toBe(false)
        expect(authorShown(null, 'offline', 'specific')).toBe(false)
        // an explicit off stays off even for an audience that would have shown it
        expect(authorShown(false, 'link', 'signed_in')).toBe(false)
        // an explicit on stays on, even in a public link or an offline file
        expect(authorShown(true, 'link', 'public')).toBe(true)
        expect(authorShown(true, 'offline', null)).toBe(true)
    })
    it('is drawn in the dialog, off until the person or the audience turns it on', () => {
        const html = renderToString(<ShareDialog open={true} onClose={() => undefined} agentId="a" topicId="t" title="T" agentName="A" />)
        expect(html).toContain(AUTHOR_LABEL)
        expect(html).toContain(AUTHOR_HELP)
        expect(html.match(/aria-checked="false"/g)).toHaveLength(3)
    })
})

describe('creating the link', () => {
    it('sends one public request carrying exactly what was previewed, and returns the link', async () => {
        const { result } = await previewOf(false)
        const link = await createLink({ agentId: 'a1', topicId: 't1', title: 'T', audience: 'public' }, result, 'https://vizier.example')
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

    it('refuses to create when no audience was chosen, and sends nothing', async () => {
        const { result } = await previewOf(false)
        // @ts-expect-error the type forbids it; a caller that gets past the type must still not publish
        await expect(createLink({ agentId: 'a1', topicId: 't1', title: 'T' }, result, 'https://vizier.example')).rejects.toThrow(/Choose who/)
        expect(create).not.toHaveBeenCalled()
    })

    it('opens with no audience selected and Create link disabled until one is chosen', () => {
        const html = renderToString(<ShareDialog open={true} onClose={() => undefined} agentId="a" topicId="t" title="T" agentName="A" />)
        expect(html.match(/<input[^>]*type="radio"/g)).toHaveLength(3)
        expect(html).not.toContain('checked=""')
        expect(html).toContain(CHOOSE_AUDIENCE)
        // no public confirmation is offered before public is chosen
        expect(html).not.toContain('without signing in.')
        expect(html).toMatch(/<button[^>]*disabled[^>]*>[^<]*Create link/)
    })

    it('renders all three audience choices when opened', () => {
        const html = renderToString(<ShareDialog open={true} onClose={() => undefined} agentId="a" topicId="t" title="T" agentName="A" />)
        expect(html).toContain('Specific people')
        expect(html).toContain('Anyone signed in on this instance')
        expect(html).toContain('Anyone with the link (public)')
    })

    it('the public confirmation wording states that anyone holding the link can read without signing in', () => {
        expect(PUBLIC_CONFIRM).toMatch(/anyone who has this link/i)
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
    it('no longer marks the conversation as shared in the header', () => {
        useSession.setState({ user: person(['owned_agents:share_conversations']) })
        const html = head({ topicId: 't', agent: { owner_id: 'u1' }, agentName: 'A', empty: false })
        expect(html).toContain('aria-label="Share this conversation"')
        expect(html).not.toContain('Shared:')
        expect(html).not.toContain('/settings/shares')
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

const sharePathOf = (id: string) => `/share/${encodeURIComponent(id)}`

describe('the dialog\'s link list', () => {
    const item = (over: Partial<ShareListItem> = {}): ShareListItem => ({
        share_id: 'abc_DEF-123',
        agent_id: 'a',
        agent_name: 'A',
        topic_id: 't',
        audience: 'public',
        people_count: 0,
        include_trail: false,
        size_bytes: 1024,
        created_at: Date.UTC(2026, 10, 1),
        revoked_at: null,
        removals: { credentials: 0, paths: 0, recall: 0, reasoning: 0, media: 0 },
        ...over,
    })
    const render = (rows: ReturnType<typeof linkRows>) =>
        renderToString(<LiveLinkList links={rows} onCopy={() => undefined} onRevoke={() => undefined} />)

    it('names who each live link lets in, when it was made, and copies any one of them', () => {
        const rows = linkRows(
            [item({ share_id: 'old_1', audience: 'public' }), item({ share_id: 'old_2', audience: 'signed_in', created_at: Date.UTC(2026, 10, 2) })],
            [],
            []
        )
        const html = render(rows)
        expect(html).toContain('Anyone with the link')
        expect(html).toContain('Anyone signed in on this instance')
        expect(html.match(/aria-label="Copy link: /g)).toHaveLength(2)
        expect(html.match(/>Copy link</g)).toHaveLength(2)
        expect(html).toContain(linkCreated(rows[0]))
        expect(linkCreated(rows[1])).toMatch(/\d{4}/)
        expect(html).toContain(sharePathOf('old_1'))
        expect(html).toContain(sharePathOf('old_2'))
    })

    it('says so plainly when the conversation has no links, instead of an empty table', () => {
        const html = render([])
        expect(html).toContain(NO_LINKS_YET)
        expect(html).not.toMatch(/>Copy link</)
        expect(html).not.toContain('<table')
    })

    it('puts a link made now first, keeps each link once, and drops a revoked one', () => {
        const existing = item({ share_id: 'old_1' })
        const made = { share_id: 'new_2', audience: 'signed_in' as const, people_count: 0, created_at: 2 }
        expect(linkRows([existing], [made], []).map((r) => r.share_id)).toEqual(['new_2', 'old_1'])
        expect(linkRows([existing], [made], ['new_2']).map((r) => r.share_id)).toEqual(['old_1'])
        expect(linkRows([existing], [linkRows([existing], [], [])[0]], [])).toHaveLength(1)
    })

    it('words a link\'s audience the way the picker does', () => {
        expect(linkAudience({ share_id: 'x', audience: 'public', people_count: 0, created_at: 0 })).toBe('Anyone with the link')
        expect(linkAudience({ share_id: 'x', audience: 'specific', people_count: 1, created_at: 0 })).toBe('Specific people (1 person)')
        expect(linkAudience({ share_id: 'x', audience: 'specific', people_count: 3, created_at: 0 })).toBe('Specific people (3 people)')
    })

    it('shows the links the header read inside the dialog, ready to copy', () => {
        const existing = { links: [item({ share_id: 'live_1', audience: 'signed_in' })], loading: false, error: null, reload: () => undefined }
        const html = renderToString(<ShareDialog open={true} onClose={() => undefined} agentId="a" topicId="t" title="T" agentName="A" existing={existing} />)
        expect(html).toContain('Links to this conversation')
        expect(html).toContain('Anyone signed in on this instance')
        expect(html).toMatch(/aria-label="Copy link: /)
        expect(html).toContain(sharePathOf('live_1'))
    })

    it('creates the row the list shows from what the server stored', async () => {
        const { result } = await previewOf(false)
        const out = await createShareFor({ agentId: 'a1', topicId: 't1', title: 'T', audience: 'public' }, result, 'https://vizier.example')
        expect(out.link).toBe('https://vizier.example/share/abc_DEF-123')
        expect(out.share.share_id).toBe('abc_DEF-123')
        expect(linkRows([], [{ share_id: out.share.share_id, audience: 'public', people_count: 0, created_at: out.share.created_at }], [])[0].share_id).toBe('abc_DEF-123')
    })
})

it('the offline tab shows the 20 MB file budget while the link tab keeps 5 MiB (FR-046)', async () => {
    const { result, preview } = await previewOf(false)
    const link = tabPreview(result, 'link')
    const file = tabPreview(result, 'offline')
    // the link tab is unchanged: same budget and same wording as before
    expect(link.limitBytes).toBe(SHARE_LIMITS.bodyBytes - SHARE_LIMITS.envelopeBytes)
    expect(link.limitBytes).toBe(preview.limitBytes)
    expect(summaryLine(link)).toContain('of 5.0 MiB')
    // the offline tab measures the same conversation against the file's own limit
    expect(file.limitBytes).toBe(SHARE_LIMITS.offlineBytes)
    expect(file.sizeBytes).toBe(link.sizeBytes)
    expect(summaryLine(file)).toContain('of 20.0 MiB')
    expect(file.canShare).toBe(true)
})

it('lets the offline tab export a 6 MB conversation that the link tab refuses', async () => {
    const { result } = await previewOf(false)
    const chunk = 'x'.repeat(100_000)
    result.snapshot.turns.push({
        id: 'big',
        role: 'agent',
        at: 1,
        parts: Array.from({ length: 64 }, () => ({ kind: 'text' as const, markdown: chunk })),
    })
    const link = tabPreview(result, 'link')
    const file = tabPreview(result, 'offline')
    expect(link.refusal?.reason).toBe('too_large')
    expect(link.canShare).toBe(false)
    expect(link.refusal?.message).toContain(formatBytes(link.sizeBytes))
    expect(link.refusal?.message).toContain('a shared conversation can be at most')
    expect(file.canShare).toBe(true)
    expect(file.refusal).toBeNull()
    expect(summaryLine(file)).toContain(`of ${formatBytes(SHARE_LIMITS.offlineBytes)}`)
    expect(summaryLine(file)).not.toContain('of 5.0 MiB')
})

it('the offline tab warns that a file cannot be revoked or limited to an audience (FR-035)', () => {
    expect(OFFLINE_WARNING).toMatch(/cannot be revoked or limited to an audience/)
})

it('the review warning (FR-020) says removal cannot recognise everything and that the person is responsible', () => {
    expect(REVIEW_WARNING).toMatch(/cannot recognise everything sensitive/)
    expect(REVIEW_WARNING).toMatch(/responsible/)
})
