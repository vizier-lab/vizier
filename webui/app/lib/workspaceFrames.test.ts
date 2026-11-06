import { describe, expect, it } from 'vitest'
import type { WorkspaceEntry } from '../api/endpoints/workspaces'
import { applyFrame, applyPage, emptyTopicView } from './workspaceFrames'

const entry = (seq: number, body = 'x'): WorkspaceEntry => ({
    seq,
    kind: 'person',
    author: { kind: 'person', id: 'rafi', name: 'rafi', available: true },
    body,
    at: seq * 1000,
    mentions: [],
    origin: null,
    is_final: false,
    activated_by_seq: null,
    history_uid: null,
    files: [],
})

describe('Workspace socket frames', () => {
    it('hello says who is working and flags a gap in what is held', () => {
        const held = applyPage(emptyTopicView(), { entries: [entry(1)], steps: [], next_before: null }, false)
        const next = applyFrame(held, { hello: { latest_seq: 4, working: [{ agent: 'alpha', phase: 'thinking' }] } })
        expect(next.working).toEqual({ alpha: true })
        expect(next.stale).toBe(true)
        expect(applyFrame(held, { hello: { latest_seq: 1, working: [] } }).stale).toBe(false)
    })

    it('adds an entry once, however often it arrives', () => {
        let s = emptyTopicView()
        s = applyFrame(s, { workspace_entry: entry(2, 'a') })
        s = applyFrame(s, { workspace_entry: entry(2, 'a again') })
        s = applyFrame(s, { workspace_entry: entry(1, 'before') })
        expect(s.entries.map((e) => e.body)).toEqual(['before', 'a again'])
        expect(s.latestSeq).toBe(2)
    })

    it('updates the mentions of one entry', () => {
        let s = applyFrame(emptyTopicView(), { workspace_entry: { ...entry(3), mentions: [{ kind: 'agent', handle: 'beta', state: 'pending' }] } })
        s = applyFrame(s, { workspace_mentions: { seq: 3, mentions: [{ kind: 'agent', handle: 'beta', state: 'unavailable' }] } })
        expect(s.entries[0].mentions[0].state).toBe('unavailable')
    })

    it('tracks a step by uid and an agent’s state', () => {
        const step = { agent_id: 'alpha', entry: { uid: 'u1', vizier_session: { agent_id: 'alpha', channel: '', topic: '' }, content: { ToolCall: { call_id: 'c', name: 'fetch', arguments: {} } } } }
        let s = applyFrame(emptyTopicView(), { history_entry: step })
        s = applyFrame(s, { history_entry: step })
        expect(s.steps).toHaveLength(1)
        s = applyFrame(s, { agent_state: { agent: 'alpha', phase: 'thinking' } })
        expect(s.working).toEqual({ alpha: true })
        s = applyFrame(s, { agent_state: { agent: 'alpha', phase: 'idle' } })
        expect(s.working).toEqual({})
    })

    it('records removal and refused posts, and ignores what it does not know', () => {
        let s = applyFrame(emptyTopicView(), { post_error: { status: 422, code: 'not_in_workspace', message: 'Side chats are not available' } })
        expect(s.postError).toMatch(/Side chats/)
        s = applyFrame(s, { workspace_entry: entry(1) })
        expect(s.postError).toBeNull()
        expect(applyFrame(s, { closed: { reason: 'removed' } }).closed).toBe('removed')
        expect(applyFrame(s, { something_new: 1 })).toBe(s)
        expect(applyFrame(s, null)).toBe(s)
    })

    it('keeps the older cursor when the newest page is read again', () => {
        let s = applyPage(emptyTopicView(), { entries: [entry(5)], steps: [], next_before: 5 }, false)
        s = applyPage(s, { entries: [entry(2)], steps: [], next_before: 2 }, true)
        expect(s.nextBefore).toBe(2)
        s = applyPage(s, { entries: [entry(5), entry(6)], steps: [], next_before: 5 }, false)
        expect(s.nextBefore).toBe(2)
    })
})
