import { describe, expect, it } from 'vitest'
import type { ChatMessage, RecallOrigin, RecalledPassage } from '../api/frames'
import { recalledFrom } from './recall'

const session = { agent_id: 'plug', channel: 'webui', topic: 't' }

const passage = (path: string, score: number): RecalledPassage => ({
    bundle: 'infra',
    path,
    ordinal: 0,
    score,
})

const recall = (uid: string, passages: RecalledPassage[], origin: RecallOrigin = 'auto'): ChatMessage => ({
    uid,
    vizier_session: session,
    content: { Recall: { origin, passages } },
})

const chat = (uid: string): ChatMessage => ({
    uid,
    vizier_session: session,
    content: { AssistantMessage: 'hi' },
})

describe('recalledFrom', () => {
    it('puts the newest recollection first and the best match first within it', () => {
        const rows = recalledFrom([
            recall('old', [passage('a.md', 0.91), passage('b.md', 0.52)]),
            chat('between'),
            recall('new', [passage('c.md', 0.83), passage('d.md', 0.64)], 'search'),
        ])
        expect(rows.map((r) => [r.uid, r.path, r.score])).toEqual([
            ['new', 'c.md', 0.83],
            ['new', 'd.md', 0.64],
            ['old', 'a.md', 0.91],
            ['old', 'b.md', 0.52],
        ])
        expect(rows[0].origin).toBe('search')
    })

    it('sorts an entry whose passages were stored out of order', () => {
        const rows = recalledFrom([
            recall('r', [passage('weak.md', 0.3), passage('best.md', 0.9), passage('mid.md', 0.6)]),
        ])
        expect(rows.map((r) => r.path)).toEqual(['best.md', 'mid.md', 'weak.md'])
    })

    it('ignores entries that are not a recollection', () => {
        expect(recalledFrom([chat('a'), chat('b')])).toEqual([])
    })
})
