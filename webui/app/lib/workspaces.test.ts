import { describe, expect, it } from 'vitest'
import type { ChatMessage } from '../api/frames'
import type { EntryMention, WorkspaceEntry, WorkspaceStep } from '../api/endpoints/workspaces'
import { buildTimeline, entriesByUid, mergeEntries, mergeSteps } from './workspaces'

const T0 = Date.parse('2026-11-06T10:00:00Z')
const at = (s: number) => T0 + s * 1000
const iso = (s: number) => new Date(at(s)).toISOString()

let seq = 0
const person = (body: string, s: number, who = 'rafi'): WorkspaceEntry => ({
    seq: (seq += 1),
    kind: 'person',
    author: { kind: 'person', id: who, name: who, available: true },
    body,
    at: at(s),
    mentions: [] as EntryMention[],
    origin: null,
    is_final: false,
    activated_by_seq: null,
    history_uid: null,
    files: [],
})
const agentEntry = (agent: string, body: string, s: number, uid: string | null, final = false): WorkspaceEntry => ({
    ...person(body, s, agent),
    kind: 'agent',
    author: { kind: 'agent', id: agent, name: agent, available: true },
    is_final: final,
    history_uid: uid,
})
const note = (body: string, s: number): WorkspaceEntry => ({ ...person(body, s, 'system'), kind: 'note' })

const step = (agent: string, uid: string, s: number, content: ChatMessage['content']): WorkspaceStep => ({
    agent_id: agent,
    entry: { uid, timestamp: iso(s), vizier_session: { agent_id: agent, channel: 'workspace__w', topic: 't' }, content },
})
const call = (agent: string, id: string, s: number) => step(agent, `call-${id}`, s, { ToolCall: { call_id: id, name: 'fetch', arguments: {} } })
const result = (agent: string, id: string, s: number) => step(agent, `res-${id}`, s, { ToolResult: { call_id: id, content: 'ok' } })
const said = (agent: string, uid: string, s: number, text: string) => step(agent, uid, s, { AssistantMessage: text })
const reply = (agent: string, uid: string, s: number, text: string) =>
    step(agent, uid, s, { Response: { timestamp: iso(s), content: { message: { content: text } } } })

const kinds = (items: ReturnType<typeof buildTimeline>) =>
    items.map((i) => (i.kind === 'run' ? `run:${i.segment.agentId}${i.segment.continued ? '+' : ''}` : i.kind))

describe('a Workspace topic as things to draw', () => {
    it('puts people, an agent turn and notes in time order', () => {
        const entries = [
            person('@alpha look', 0),
            agentEntry('alpha', 'on it', 3, 'u-say'),
            agentEntry('alpha', 'done', 6, 'u-done', true),
            note('rafi started a checkpoint for Alpha', 8),
        ]
        const steps = [call('alpha', 'c1', 1), result('alpha', 'c1', 2), said('alpha', 'u-say', 3, 'on it'), reply('alpha', 'u-done', 6, 'done')]
        const items = buildTimeline(entries, steps)
        expect(kinds(items)).toEqual(['message', 'run:alpha', 'note'])
        const run = items[1]
        if (run.kind !== 'run') throw new Error('expected a run')
        // trail, the mid-turn message, then the final reply as the outcome
        expect(run.segment.blocks.map((b) => b.kind)).toEqual(['trail', 'message'])
        expect(run.segment.outcome?.uid).toBe('u-done')
        expect(run.segment.last).toBe(true)
    })

    it('does not draw an agent entry twice: its step draws it', () => {
        const items = buildTimeline([agentEntry('alpha', 'hi', 1, 'u1', true)], [reply('alpha', 'u1', 1, 'hi')])
        expect(kinds(items)).toEqual(['run:alpha'])
    })

    it('splits a turn where someone else spoke, and repeats the head after it', () => {
        const entries = [person('@alpha go', 0), person('also, maya is out', 4, 'maya')]
        const steps = [call('alpha', 'c1', 1), result('alpha', 'c1', 2), said('alpha', 'u-say', 6, 'back'), reply('alpha', 'u-done', 8, 'finished')]
        const items = buildTimeline([...entries, agentEntry('alpha', 'back', 6, 'u-say'), agentEntry('alpha', 'finished', 8, 'u-done', true)], steps)
        expect(kinds(items)).toEqual(['message', 'run:alpha', 'message', 'run:alpha+'])
        const [first, second] = items.filter((i) => i.kind === 'run')
        if (first.kind !== 'run' || second.kind !== 'run') throw new Error('runs')
        expect(first.segment.last).toBe(false)
        expect(second.segment.last).toBe(true)
        expect(second.segment.outcome?.uid).toBe('u-done')
    })

    it('keeps two agents working at once as two runs', () => {
        const entries = [person('@alpha @beta tools', 0)]
        const steps = [call('alpha', 'a', 1), reply('alpha', 'a-done', 5, 'A'), call('beta', 'b', 2), reply('beta', 'b-done', 4, 'B')]
        const items = buildTimeline(
            [...entries, agentEntry('alpha', 'A', 5, 'a-done', true), agentEntry('beta', 'B', 4, 'b-done', true)],
            steps
        )
        // Beta's final reply lands inside alpha's turn, so alpha's run is split around it.
        expect(kinds(items).filter((k) => k.startsWith('run:beta'))).toEqual(['run:beta'])
        expect(kinds(items).filter((k) => k.startsWith('run:alpha')).length).toBeGreaterThanOrEqual(1)
    })

    it('puts a mention_agent message, which has no step, into its author’s turn at its time', () => {
        const entries = [person('@alpha ask beta', 0), agentEntry('alpha', '@beta tools', 3, null)]
        const steps = [call('alpha', 'c1', 1), result('alpha', 'c1', 2), reply('alpha', 'u-done', 5, 'asked')]
        const items = buildTimeline([...entries, agentEntry('alpha', 'asked', 5, 'u-done', true)], steps)
        const run = items.find((i) => i.kind === 'run')
        if (run?.kind !== 'run') throw new Error('expected a run')
        expect(run.segment.blocks.map((b) => b.kind)).toEqual(['trail', 'message'])
        const message = run.segment.blocks[1]
        expect(message.kind === 'message' && message.text).toBe('@beta tools')
    })

    it('draws a shown message whose step is not on the page', () => {
        const items = buildTimeline([agentEntry('alpha', 'earlier answer', 1, 'gone', true)], [])
        const run = items[0]
        if (run.kind !== 'run') throw new Error('expected a run')
        expect(run.segment.outcome?.content.Response).toBeTruthy()
    })

    it('leaves out an agent’s checkpoint-free internals it does not show', () => {
        const items = buildTimeline([], [step('alpha', 'r', 1, { Recall: { origin: 'auto' as never, passages: [] } })])
        expect(items).toEqual([])
    })
})

describe('holding pages and live frames', () => {
    it('merges entries by seq, replacing a known one', () => {
        const a = person('a', 1)
        const b = person('b', 2)
        const merged = mergeEntries([b], [a, { ...b, body: 'b2' }])
        expect(merged.map((e) => e.body)).toEqual(['a', 'b2'])
    })

    it('merges steps by uid', () => {
        const first = call('alpha', 'c1', 1)
        const merged = mergeSteps([first], [first, result('alpha', 'c1', 2)])
        expect(merged).toHaveLength(2)
    })

    it('looks entries up by the history row they came from', () => {
        const e = agentEntry('alpha', 'x', 1, 'u1')
        const noStep = agentEntry('alpha', 'y', 2, null)
        const map = entriesByUid([e, noStep])
        expect(map.get('u1')).toBe(e)
        expect(map.get(`entry-${noStep.seq}`)).toBe(noStep)
    })
})
