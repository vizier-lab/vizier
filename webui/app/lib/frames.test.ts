import { describe, expect, it } from 'vitest'
import type { ChatMessage, Frame, WebSocketResponse } from '../api/frames'
import { emptyTopic, nextQueued, reduce, type Action, type TopicState } from './frames'
import { groupHistory, openTurn } from './trail'

const session = { agent_id: 'plug', channel: 'webui', topic: 't' }
const frame = (f: Frame): Action => ({ type: 'frame', frame: f })
const response = (content: WebSocketResponse['content'], extra: Partial<WebSocketResponse> = {}): Action =>
    frame({ kind: 'response', response: { timestamp: '2026-10-23T10:00:00Z', content, ...extra } })
const entry = (uid: string, content: ChatMessage['content'], timestamp = '2026-10-23T10:00:00Z'): ChatMessage => ({
    uid, timestamp, vizier_session: session, content,
})
const run = (state: TopicState, ...actions: Action[]) => actions.reduce(reduce, state)
const start = () => emptyTopic(session)

const request = (uid = 'r1') => entry(uid, { Request: { timestamp: 't', user: 'me', content: { chat: 'hi' } } })
const call = (uid: string, id: string) =>
    entry(uid, { ToolCall: { call_id: id, name: 'execute_python', arguments: { intent: 'x', code: 'y' } } })
const result = (uid: string, id: string) => entry(uid, { ToolResult: { call_id: id, content: '{"ok":true}' } })

describe('turn state machine', () => {
    it('goes Sent → Thinking → Working → Answered', () => {
        let s = run(start(), { type: 'send', id: '1', text: 'hello' })
        expect(s.phase).toBe('thinking')
        expect(s.entries).toHaveLength(1)

        s = run(s, response('thinking_start'), response({ tool_choice: { name: 'execute_python', args: {} } }))
        expect(s.phase).toBe('working')
        expect(s.label).toBe('execute_python')

        s = run(s, response({ message: { content: 'done', stats: { current_context_size: 21000, context_window: 200000 } as never } }, { history_uid: 'rep1' }))
        expect(s.phase).toBe('idle')
        expect(s.label).toBeNull()
        expect(s.context).toEqual({ used: 21000, window: 200000 })
        expect(s.entries.at(-1)?.uid).toBe('rep1')
    })

    it('holds a message sent while busy and sends it when the turn ends', () => {
        let s = run(start(), { type: 'send', id: '1', text: 'first' }, response('thinking_start'))
        s = run(s, { type: 'send', id: '2', text: 'second' })
        expect(s.queued.map((q) => q.text)).toEqual(['second'])
        expect(nextQueued(s)).toBeUndefined()

        s = run(s, response({ message: { content: 'ok' } }, { history_uid: 'a' }))
        expect(nextQueued(s)?.text).toBe('second')
        s = run(s, { type: 'dequeue' })
        expect(s.queued).toEqual([])
        expect(s.phase).toBe('thinking')
    })

    it('drops the queue on abort', () => {
        let s = run(start(), { type: 'send', id: '1', text: 'a' }, response('thinking_start'), { type: 'send', id: '2', text: 'b' })
        s = run(s, { type: 'abort' })
        expect(s.queued).toEqual([])
        expect(s.phase).toBe('idle')
        expect(s.stopped).toBe(true)
        s = run(s, { type: 'send', id: '3', text: 'c' })
        expect(s.stopped).toBe(false)
    })

    it('keeps an error for its Notice and goes idle', () => {
        const s = run(start(), response('thinking_start'), response({ error: { kind: 'rate_limited', message: 'slow down' } }))
        expect(s.phase).toBe('idle')
        expect(s.error).toEqual({ kind: 'rate_limited', message: 'slow down' })
    })

    it('marks a resumed reply', () => {
        const s = run(start(), response('thinking_start'), response({ message: { content: 'back' } }, { resumed: true, history_uid: 'x' }))
        expect(s.resumed).toBe(true)
    })
})

describe('history and reopening', () => {
    it('shows a turn in progress after a reopen, with no duplicate steps', () => {
        // The page loads history that already holds the request and one step...
        let s = run(start(), { type: 'history', entries: [request(), call('c1', 'k1')] }, { type: 'busy', thinking: true })
        expect(s.phase).toBe('working')
        // ...then the same step is pushed again, and a new one.
        s = run(s, frame({ kind: 'history_entry', entry: call('c1', 'k1') }), frame({ kind: 'history_entry', entry: result('r1x', 'k1') }))
        expect(s.entries.map((e) => e.uid)).toEqual(['r1', 'c1', 'r1x'])
        const turns = groupHistory(s.entries)
        expect(turns).toHaveLength(1)
        expect(openTurn(turns)).toBeDefined()
        expect(turns[0].trail).toHaveLength(1)
    })

    it('treats steps of a turn nobody here started as work in progress', () => {
        const s = run(start(), frame({ kind: 'history_entry', entry: call('c1', 'k1') }))
        expect(s.phase).toBe('working')
    })

    it('replaces the stand-ins drawn while the turn ran once history is read back', () => {
        let s = run(start(), { type: 'send', id: '1', text: 'hi' }, response({ message: { content: 'yo' } }))
        expect(s.entries.every((e) => e.uid.startsWith('live-'))).toBe(true)
        const stored = [
            entry('r1', { Request: { timestamp: 't', user: 'me', content: { chat: 'hi' } } }),
            entry('a1', { Response: { timestamp: '2026-10-23T10:00:00Z', content: { message: { content: 'yo' } } } }),
        ]
        s = run(s, { type: 'history', entries: stored })
        expect(s.entries.map((e) => e.uid)).toEqual(['r1', 'a1'])
    })
})

describe('side frames', () => {
    it('keeps jobs by id and replaces a changed one', () => {
        const job = (state: string) => ({ id: 'j1', kind: 'batch', state, pieces: [] }) as never
        const s = run(start(), frame({ kind: 'job', snapshot: job('running') }), frame({ kind: 'job', snapshot: job('reported') }))
        expect(Object.keys(s.jobs)).toEqual(['j1'])
        expect(s.jobs.j1.state).toBe('reported')
    })

    it('prepends a new session file and replaces a relabelled one', () => {
        const file = (id: string, sender: string) => ({ id, filename: id, sender }) as never
        let s = run(start(), frame({ kind: 'session_file', file: file('a', 'plug') }), frame({ kind: 'session_file', file: file('b', 'plug') }))
        expect(s.files.map((f) => f.id)).toEqual(['b', 'a'])
        s = run(s, frame({ kind: 'session_file', file: file('a', 'me') }))
        expect(s.files.map((f) => f.id)).toEqual(['a', 'b'])
        expect(s.files[0].sender).toBe('me')
    })

    it('sets reactions on the entry they name', () => {
        const s = run(
            start(),
            { type: 'history', entries: [entry('a1', { Response: { timestamp: 't', content: { message: { content: 'yo' } } } })] },
            frame({ kind: 'reactions', messageUid: 'a1', reactions: [{ user_id: 'u', emoji: '👍' } as never] })
        )
        expect(s.entries[0].reactions).toHaveLength(1)
    })
})

describe('paging history', () => {
    it('puts an older page above what is held, in stored order', () => {
        const a = entry('a', { AssistantMessage: 'a' }), b = entry('b', { AssistantMessage: 'b' })
        const c = entry('c', { AssistantMessage: 'c' }), d = entry('d', { AssistantMessage: 'd' })
        let s = run(start(), { type: 'history', entries: [c, d] })
        s = run(s, { type: 'history', entries: [a, b], position: 'older' })
        expect(s.entries.map((e) => e.uid)).toEqual(['a', 'b', 'c', 'd'])
    })

    it('replaces only the newest page when it is re-read, keeping older entries above it', () => {
        const [a, b, c, d] = ['a', 'b', 'c', 'd'].map((u) => entry(u, { AssistantMessage: u }))
        let s = run(start(), { type: 'history', entries: [a, b, c] })
        s = run(s, { type: 'history', entries: [b, c, d] })
        expect(s.entries.map((e) => e.uid)).toEqual(['a', 'b', 'c', 'd'])
    })
})
