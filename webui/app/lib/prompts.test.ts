import { describe, expect, it } from 'vitest'
import type { ChatMessage } from '../api/frames'
import { currentPrompt, markOf, previewOf, promptIndex, withWaiting, type PromptMark } from './prompts'

const entry = (uid: string, timestamp: string, content: object, kind: 'Request' | 'Response' = 'Request'): ChatMessage =>
    ({
        uid,
        seq: null,
        timestamp,
        vizier_session: { agent_id: 'a', channel: 'c', topic: 't' },
        content: kind === 'Request' ? { Request: { timestamp, user: 'me', content, metadata: null } } : { Response: { content } },
    }) as unknown as ChatMessage

const mark = (uid: string, timestamp: string): PromptMark => ({ uid, seq: null, timestamp, preview: uid })

describe('markOf', () => {
    it('takes what a person sent and leaves out what a machine did', () => {
        expect(markOf(entry('a', '2026-11-18T10:00:00Z', { chat: '\n hello there\nmore' }))?.preview).toBe('hello there')
        expect(markOf(entry('b', '2026-11-18T10:00:00Z', { client_event: { text: 'pressed Save' } }))?.preview).toBe('pressed Save')
        expect(markOf(entry('c', '2026-11-18T10:00:00Z', { audio_chat: [{}, null] }))?.preview).toBe('Voice message')
        expect(markOf(entry('d', '2026-11-18T10:00:00Z', { detached_delivery: 'x' }))).toBeNull()
        expect(markOf(entry('e', '2026-11-18T10:00:00Z', { command: 'abort' }))).toBeNull()
        expect(markOf(entry('f', '2026-11-18T10:00:00Z', { chat: 'x' }, 'Response'))).toBeNull()
    })
})

describe('previewOf', () => {
    it('cuts a long line with an ellipsis', () => {
        const p = previewOf('x'.repeat(300))
        expect(p).toHaveLength(120)
        expect(p.endsWith('…')).toBe(true)
    })
})

describe('promptIndex', () => {
    const server = [mark('p1', '2026-11-18T10:00:00Z'), mark('p2', '2026-11-18T10:05:00Z')]

    it('adds a prompt sent since the list was read, once', () => {
        const entries = [
            entry('p2', '2026-11-18T10:05:00Z', { chat: 'two' }),
            entry('live-1', '2026-11-18T10:09:00Z', { chat: 'three' }),
        ]
        expect(promptIndex(server, entries).map((m) => m.uid)).toEqual(['p1', 'p2', 'live-1'])
    })

    it('adds a message just sent when the server writes another offset', () => {
        // 23:05 in Jakarta is 16:05 UTC; as text the stand-in would read as older.
        const local = [mark('p1', '2026-11-18T23:00:00.000+07:00')]
        const entries = [entry('live-1', '2026-11-18T16:05:00.000Z', { chat: 'new' })]
        expect(promptIndex(local, entries).map((m) => m.uid)).toEqual(['p1', 'live-1'])
    })

    it('keeps the list as it is when nothing is new', () => {
        const entries = [entry('p2', '2026-11-18T10:05:00Z', { chat: 'two' })]
        expect(promptIndex(server, entries)).toBe(server)
    })
})

describe('currentPrompt', () => {
    const index = ['a', 'b', 'c', 'd'].map((uid, i) => mark(uid, `2026-11-18T10:0${i}:00Z`))

    it('is the last prompt above the reading line', () => {
        const tops = new Map([['b', -300], ['c', -20], ['d', 400]])
        expect(currentPrompt(index, tops)).toBe(2)
    })

    it('is the first loaded prompt when the reader is above all of them', () => {
        const tops = new Map([['c', 50], ['d', 400]])
        expect(currentPrompt(index, tops)).toBe(2)
    })

    it('is the newest prompt when none is on the page', () => {
        expect(currentPrompt(index, new Map())).toBe(3)
    })
})

describe('withWaiting', () => {
    it('puts messages sent while the agent was busy last', () => {
        const index = [mark('p1', '2026-11-18T10:00:00Z')]
        const out = withWaiting(index, [{ id: '7', text: 'and one more\nthing', sentAt: '2026-11-18T10:01:00Z' }])
        expect(out.map((m) => [m.uid, m.preview])).toEqual([['p1', 'p1'], ['q-7', 'and one more']])
        expect(withWaiting(index, [])).toBe(index)
    })
})
