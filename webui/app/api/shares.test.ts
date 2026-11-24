import { afterEach, describe, expect, it, vi } from 'vitest'
import { SHARE_LIMITS } from '../lib/share/limits'
import { liveSharesOf, sharesOfConversation, type ShareListItem } from './shares'

const item = (n: number, over: Partial<ShareListItem> = {}): ShareListItem => ({
    share_id: `s${n}`,
    title: 'T',
    agent_id: 'a1',
    agent_name: 'Atlas',
    topic_id: 't1',
    audience: 'public',
    people_count: 0,
    include_trail: false,
    size_bytes: 1,
    created_at: 1000 - n,
    revoked_at: null,
    removals: { credentials: 0, paths: 0, recall: 0, reasoning: 0, media: 0 },
    ...over,
})

afterEach(() => vi.unstubAllGlobals())

describe('shares of one conversation (FR-038)', () => {
    it('keeps live shares of that agent and topic only', () => {
        const all = [item(1), item(2, { revoked_at: 5 }), item(3, { topic_id: 't2' }), item(4, { agent_id: 'a2' })]
        expect(liveSharesOf(all, 'a1', 't1').map((i) => i.share_id)).toEqual(['s1'])
    })

    it('pages through the list with `before` until a short page', async () => {
        const urls: string[] = []
        const first = Array.from({ length: 100 }, (_, i) => item(i, i === 99 ? { topic_id: 't1' } : { topic_id: 'other' }))
        const second = [item(200)]
        vi.stubGlobal('localStorage', { getItem: () => null, setItem: () => undefined, removeItem: () => undefined })
        vi.stubGlobal('fetch', async (url: string) => {
            urls.push(String(url))
            return new Response(JSON.stringify({ status: 200, data: urls.length === 1 ? first : second }), { status: 200 })
        })
        const found = await sharesOfConversation('a1', 't1')
        expect(found.map((i) => i.share_id)).toEqual(['s99', 's200'])
        expect(urls).toHaveLength(2)
        expect(urls[1]).toContain(`before=${first[99].created_at}`)
    })
})

describe('limits agree with the spec', () => {
    it('failed opens 60/min, 10 people matches, 30 people searches/min, 50 named people', () => {
        expect(SHARE_LIMITS.missPerMinute).toBe(60)
        expect(SHARE_LIMITS.peopleResults).toBe(10)
        expect(SHARE_LIMITS.peopleSearchesPerMinute).toBe(30)
        expect(SHARE_LIMITS.people).toBe(50)
    })
})
