import { describe, expect, it } from 'vitest'
import type { ProjectSessionRow } from '../api/endpoints/projects'
import { mostRecentSession, projectSessionRows } from './projectSessions'

const row = (topic: string, at: number | null, worktree = false): ProjectSessionRow => ({
    topic,
    title: topic,
    is_thinking: false,
    last_activity: at,
    worktree: worktree ? { id: `w-${topic}`, name: topic, branch: topic, available: true } : null,
})

// Eight plain sessions plus one worktree chat, in creation order, newest first.
const eight = ['t1', 't2', 't3', 't4', 't5', 't6', 't7', 't8'].map((t, i) => row(t, 100 - i))
const withWorktree = [row('w1', 1, true), ...eight]

describe('projectSessionRows', () => {
    it('shows five of the non-worktree sessions, and says how many it left out', () => {
        const { rows, hidden } = projectSessionRows(eight, false)
        expect(rows.map((r) => r.topic)).toEqual(['t1', 't2', 't3', 't4', 't5'])
        expect(hidden).toBe(3)
    })

    it('shows them all when asked, and still counts them', () => {
        const { rows, hidden, total } = projectSessionRows(eight, true)
        expect(rows).toHaveLength(8)
        expect(hidden).toBe(0)
        // `total` does not follow `hidden` down: it is what keeps "Show less" on screen after the
        // click that hid nothing.
        expect(total).toBe(8)
    })

    it('lifts the worktree chats to the top and never hides them', () => {
        // The worktree chat is the oldest, and still first: a checkout is not a chat to file away.
        const { rows, hidden } = projectSessionRows(withWorktree, false)
        expect(rows[0].topic).toBe('w1')
        expect(rows.slice(1).map((r) => r.topic)).toEqual(['t1', 't2', 't3', 't4', 't5'])
        expect(hidden).toBe(3)
        expect(projectSessionRows(withWorktree, true).rows.map((r) => r.topic).slice(0, 2)).toEqual(['w1', 't1'])
    })

    it('keeps the order it was given inside each part', () => {
        const mixed = [row('a', 1), row('wt', 2, true), row('b', 3), row('wt2', 4, true), row('c', 5)]
        expect(projectSessionRows(mixed, true).rows.map((r) => r.topic)).toEqual(['wt', 'wt2', 'a', 'b', 'c'])
    })

    it('leaves a short list alone', () => {
        const three = eight.slice(0, 3)
        expect(projectSessionRows(three, false)).toEqual({ rows: three, hidden: 0, total: 3 })
        expect(projectSessionRows([], false)).toEqual({ rows: [], hidden: 0, total: 0 })
    })
})

describe('mostRecentSession', () => {
    it('picks the one used last, not the newest and not an empty one', () => {
        const sessions = [row('newest', null), row('busy', 6_000), row('used', 9_000)]
        expect(mostRecentSession(sessions)?.topic).toBe('used')
    })

    it('falls back to the first row when nothing has run a turn', () => {
        expect(mostRecentSession([row('a', null), row('b', null)])?.topic).toBe('a')
        expect(mostRecentSession([])).toBeUndefined()
    })
})
