import { describe, expect, it } from 'vitest'
import type { SetupState } from '../api/endpoints/agents'
import type { NotificationItem } from '../api/endpoints/notifications'
import {
    TERMINAL_MS,
    absorb,
    dropTerminal,
    isTerminal,
    onHello,
    onHelloSetups,
    onRead,
    onSetup,
    onSetupCleared,
    parseFrame,
    type InboxNotes,
} from './inboxFrames'

const counts = { all: 3, unread: 2, read: 1 }

function item(id: number, over: Partial<NotificationItem> = {}): NotificationItem {
    return { id, at: '2026-11-06T09:00:00Z', agent_id: 'a', category: 'task_run', tone: 'default', text: `n${id}`, href: '/a/tasks', read: false, ...over }
}

function notes(over: Partial<InboxNotes> = {}): InboxNotes {
    return { open: false, filter: 'all', items: [], counts, loaded: false, lastSeenId: 5, primed: true, arrived: [], viewing: null, ...over }
}

function setup(over: Partial<SetupState> = {}): SetupState {
    return { agent_id: 'scout', phase: 'downloading', message: 'bge', downloads: [{ kind: 'embedding', completed: 50, total: 100 }], ...over }
}

describe('parseFrame', () => {
    it('accepts each known frame and ignores unknown types and junk', () => {
        expect(parseFrame({ type: 'resync' })).toEqual({ type: 'resync' })
        expect(parseFrame({ type: 'setup_cleared', agent_id: 'a' })).not.toBeNull()
        expect(parseFrame({ type: 'setup', state: setup() })).not.toBeNull()
        expect(parseFrame({ type: 'read', read: true, counts })).not.toBeNull()
        expect(parseFrame({ type: 'notification', item: item(1), counts })).not.toBeNull()
        expect(parseFrame({ type: 'hello', counts, latest_id: 1, setups: [] })).not.toBeNull()
        expect(parseFrame({ type: 'from_the_future' })).toBeNull()
        expect(parseFrame({ type: 'setup' })).toBeNull()
        expect(parseFrame('x')).toBeNull()
        expect(parseFrame(null)).toBeNull()
    })
})

describe('absorb', () => {
    it('treats rows above the mark as new and rows at or below it as old', () => {
        const { patch } = absorb(notes(), [item(6), item(5), item(4)], counts)
        expect(patch.arrived?.map((i) => i.id)).toEqual([6])
        expect(patch.lastSeenId).toBe(6)
    })
    it('reads a reply to the open conversation without a toast and fixes the counts', () => {
        const reply = item(6, { category: 'chat_reply', href: '/a/chat/t' })
        const r = absorb(notes({ viewing: '/a/chat/t' }), [reply], counts)
        expect(r.markRead).toEqual([6])
        expect(r.patch.arrived).toEqual([])
        expect(r.patch.counts).toEqual({ all: 3, unread: 1, read: 2 })
    })
    it('raises no toast while the panel is open and keeps a loaded list current', () => {
        const r = absorb(notes({ open: true, loaded: true, items: [item(5)] }), [item(6)], counts)
        expect(r.patch.arrived).toEqual([])
        expect(r.patch.items?.map((i) => i.id)).toEqual([6, 5])
    })
    it('never counts the first snapshot as news', () => {
        const r = absorb(notes({ primed: false, lastSeenId: 0 }), [item(3), item(2)], counts)
        expect(r.patch.arrived).toBeUndefined()
        expect(r.patch.lastSeenId).toBe(3)
        expect(r.patch.primed).toBe(true)
    })
})

describe('onRead and onHello', () => {
    it('applies one read and mark-all', () => {
        const s = notes({ filter: 'unread', items: [item(2), item(1)] })
        expect(onRead(s, 2, true, counts).items?.map((i) => i.id)).toEqual([1])
        expect(onRead(s, undefined, true, counts).items).toEqual([])
    })
    it('the first hello primes; a later one asks for a page only when behind or loaded', () => {
        const first = onHello(notes({ primed: false, lastSeenId: 0 }), { counts, latest_id: 9 })
        expect(first.fetchPage).toBe(false)
        expect(first.patch.lastSeenId).toBe(9)
        expect(onHello(notes(), { counts, latest_id: 5 }).fetchPage).toBe(false)
        expect(onHello(notes(), { counts, latest_id: 6 }).fetchPage).toBe(true)
        expect(onHello(notes({ loaded: true }), { counts, latest_id: 5 }).fetchPage).toBe(true)
    })
})

describe('setup frames', () => {
    const empty = { setups: {}, samples: {} }
    it('records the state and ETA samples, and clears them', () => {
        let s = onSetup(empty, setup(), 1000)
        s = onSetup(s, setup({ downloads: [{ kind: 'embedding', completed: 80, total: 100 }] }), 3000)
        expect(s.samples.scout).toHaveLength(2)
        expect(s.setups.scout.phase).toBe('downloading')
        expect(onSetupCleared(s, 'scout')).toEqual(empty)
    })
    it('a snapshot replaces every setup and drops samples of agents no longer listed', () => {
        const s = onSetup(empty, setup({ agent_id: 'old' }), 1)
        const next = onHelloSetups(s, [setup()], 2)
        expect(Object.keys(next.setups)).toEqual(['scout'])
        expect(next.samples.old).toBeUndefined()
    })
    it('drops a finished setup after the delay only if it is still the held one', () => {
        const done = setup({ phase: 'ready', downloads: [] })
        expect(isTerminal(done)).toBe(true)
        expect(TERMINAL_MS).toBe(5000)
        const s = onSetup(empty, done, 1)
        expect(dropTerminal(s, done)?.setups).toEqual({})
        const restarted = setup()
        expect(dropTerminal(onSetup(s, restarted, 2), done)).toBeNull()
    })
})

describe('the language-server frame', () => {
    const snapshot = { project_id: 'p1', enabled: true, capacity: { in_use: 1, max: 6 }, servers: [] }

    it('is understood when it names a project and carries a snapshot', () => {
        expect(parseFrame({ type: 'lsp', project_id: 'p1', snapshot })).toEqual({ type: 'lsp', project_id: 'p1', snapshot })
    })

    it('is ignored when it is malformed', () => {
        expect(parseFrame({ type: 'lsp', snapshot })).toBeNull()
        expect(parseFrame({ type: 'lsp', project_id: 'p1', snapshot: { servers: 'x' } })).toBeNull()
    })
})
