// Unit tests for the pure trail producers. Run with the Node test runner, which strips the
// types natively, so this needs no test framework and no new dependency:
//
//   cd webui && node --test app/lib/trail.test.ts
//
// The guarantee numbers are from
// `specs/010-webui-reasoning-display/contracts/trail-model.md` §3.

import test from 'node:test'
import assert from 'node:assert/strict'

import { appendLiveEvent, groupHistory, liveEvent, trailSummary } from './trail.ts'
import type { ChatMessage, TrailEvent } from '../interfaces/types.ts'

let nextUid = 0
const entry = (content: ChatMessage['content']): ChatMessage => ({
  uid: `uid-${(nextUid += 1)}`,
  vizier_session: { agent_id: 'a', channel: 'http__someone__webui', topic: 'General' },
  content,
})

const request = (chat: string) =>
  entry({ Request: { timestamp: '2026-10-03T00:00:00Z', user: 'someone', content: { chat } } })

const response = (text: string) =>
  entry({
    Response: {
      timestamp: '2026-10-03T00:00:01Z',
      content: { message: { content: text } },
    },
  })

const think = (thought: string) =>
  entry({ ToolCall: { call_id: `c-${(nextUid += 1)}`, name: 'think', arguments: { thought } } })

const tool = (name: string, args: Record<string, unknown> = {}) =>
  entry({ ToolCall: { call_id: `c-${(nextUid += 1)}`, name, arguments: args } })

const python = (call_id: string, args: Record<string, unknown>) =>
  entry({ ToolCall: { call_id, name: 'execute_python', arguments: args } })

const toolResult = (call_id: string, content: string) => entry({ ToolResult: { call_id, content } })

const kinds = (trail: TrailEvent[]) => trail.map((e) => e.kind)

// T1 — the storage layer already ordered the entries; the grouping must not re-derive one.
test('T1 entry order in equals event order out', () => {
  const turns = groupHistory([
    request('what changed?'),
    think('check the archives'),
    tool('memory_search', { query: 'rust' }),
    python('p1', { intent: 'count the hits', code: 'len(hits)' }),
    tool('fetch', { url: 'https://example.com' }),
    response('two releases'),
  ])

  assert.equal(turns.length, 1)
  assert.deepEqual(kinds(turns[0].trail), ['thought', 'tool', 'python', 'tool'])
  assert.equal((turns[0].trail[1] as { name: string }).name, 'memory_search')
  assert.equal((turns[0].trail[3] as { name: string }).name, 'fetch')
})

// T2 — a turn with nothing on the way to its answer has an empty trail, so the renderer has
// no trail element to draw.
test('T2 an empty trail yields no trail element', () => {
  const turns = groupHistory([request('hello'), response('hi')])

  assert.equal(turns.length, 1)
  assert.deepEqual(turns[0].trail, [])
})

// T3 — adjacent reasoning reads as one block, not as a list of fragments.
test('T3 adjacent thoughts merge, and so does adjacent narration', () => {
  const turns = groupHistory([
    request('why?'),
    think('first'),
    think('second'),
    tool('memory_search', { query: 'x' }),
    think('third'),
    entry({ AssistantMessage: 'Let me check.' }),
    entry({ AssistantMessage: 'One moment.' }),
    response('because'),
  ])

  assert.deepEqual(kinds(turns[0].trail), ['thought', 'tool', 'thought', 'narration'])
  assert.equal((turns[0].trail[0] as { text: string }).text, 'first\n\nsecond')
  assert.equal((turns[0].trail[3] as { text: string }).text, 'Let me check.\n\nOne moment.')
})

// T11 — an agent-initiated turn (a scheduled task, a dream cycle) has no request. Its trail
// belongs to a turn rather than being dropped.
test('T11 an entry with no open turn yields a turn with request undefined', () => {
  const turns = groupHistory([think('unprompted'), response('done')])

  assert.equal(turns.length, 1)
  assert.equal(turns[0].request, undefined)
  assert.deepEqual(kinds(turns[0].trail), ['thought'])
  assert.ok(turns[0].outcome)
})

// T8 — one entry kind this version does not know about must not blank the conversation.
test('T8 an unrecognised entry kind is skipped without throwing', () => {
  const unknown = entry({ SomethingNew: { whatever: true } } as unknown as ChatMessage['content'])
  const turns = groupHistory([request('hi'), unknown, think('still here'), response('hi back')])

  assert.equal(turns.length, 1)
  assert.deepEqual(kinds(turns[0].trail), ['thought'])
  assert.ok(turns[0].outcome)
})

// T7 — a turn that ends in an error still has its trail, attached to that error.
test('T7 a turn whose outcome is an error keeps its trail', () => {
  const failed = entry({
    Response: {
      timestamp: '2026-10-03T00:00:01Z',
      content: { error: { kind: 'tool_timeout', message: 'timed out' } },
    },
  })
  const turns = groupHistory([request('go'), think('trying'), failed])

  assert.deepEqual(kinds(turns[0].trail), ['thought'])
  assert.ok(turns[0].outcome?.content.Response)
})

// A python run pairs with its report by call_id, exactly, when read from history.
test('a python run takes its intent from the call and its report from the matching result', () => {
  const report = { ok: true, result: 3, stdout: '', tool_calls: [], duration_ms: 12 }
  const turns = groupHistory([
    request('compute'),
    python('call-7', { intent: 'count the samples', code: 'len(xs)' }),
    toolResult('call-7', JSON.stringify(report)),
    response('three'),
  ])

  const event = turns[0].trail[0]
  assert.equal(event.kind, 'python')
  if (event.kind !== 'python') return
  assert.equal(event.intent, 'count the samples')
  assert.equal(event.code, 'len(xs)')
  assert.deepEqual(event.report, report)
})

// T9's data half: a run recorded before `intent` existed carries none, and the model says so
// rather than inventing one.
test('a python run recorded without an intent reports intent null', () => {
  const turns = groupHistory([request('compute'), python('call-8', { code: '1 + 1' })])

  const event = turns[0].trail[0]
  assert.equal(event.kind, 'python')
  if (event.kind !== 'python') return
  assert.equal(event.intent, null)
  assert.equal(event.report, null)
})

// Rule 4: a divider belongs to no turn, and closes the one that was open.
test('checkpoint and command entries belong to no turn', () => {
  const turns = groupHistory([
    request('one'),
    think('a'),
    response('first'),
    entry({ Checkpoint: { handover: 'saved', timestamp: '2026-10-03T00:00:02Z' } }),
    entry({ Command: 'lobotomy' }),
    request('two'),
    think('b'),
    response('second'),
  ])

  assert.equal(turns.length, 2)
  assert.deepEqual(kinds(turns[0].trail), ['thought'])
  assert.deepEqual(kinds(turns[1].trail), ['thought'])
})

// T10 is structural: there is no variant for a nested call, so no producer can emit one.
test('T10 no producer emits a nested tool call', () => {
  const report = {
    ok: true,
    result: null,
    stdout: '',
    tool_calls: [{ seq: 1, name: 'memory_search', ok: true, duration_ms: 3 }],
    duration_ms: 20,
  }
  const turns = groupHistory([
    request('loop it'),
    python('call-9', { intent: 'search twelve times', code: 'for _ in range(12): search()' }),
    toolResult('call-9', JSON.stringify(report)),
  ])

  assert.deepEqual(kinds(turns[0].trail), ['python'])
})

// The live producer: same kinds, same merging, and a report paired positionally because
// `ToolChoice` carries no correlation id.
test('the live stream produces the same kinds and pairs a report positionally', () => {
  let trail: TrailEvent[] = []
  trail = appendLiveEvent(trail, { thinking: 'first' }, 'e1')
  trail = appendLiveEvent(trail, { thinking: 'second' }, 'e2')
  trail = appendLiveEvent(
    trail,
    { tool_choice: { name: 'execute_python', args: { intent: 'add', code: '1+1' } } },
    'e3'
  )
  trail = appendLiveEvent(
    trail,
    { tool_response: { response: { ok: true, result: 2, stdout: '', tool_calls: [], duration_ms: 4 } } },
    'e4'
  )
  trail = appendLiveEvent(trail, 'thinking_start', 'e5')

  assert.deepEqual(kinds(trail), ['thought', 'python'])
  assert.equal((trail[0] as { text: string }).text, 'first\n\nsecond')
  const run = trail[1]
  assert.equal(run.kind, 'python')
  if (run.kind !== 'python') return
  assert.equal(run.intent, 'add')
  assert.equal(run.report?.result, 2)
})

test('frames that are not trail events produce nothing', () => {
  for (const frame of ['thinking_start', 'empty', 'abort'] as const) {
    assert.equal(liveEvent(frame, 'id'), null)
  }
  assert.equal(liveEvent({ message: { content: 'done' } }, 'id'), null)
  assert.equal(liveEvent({ checkpoint: { handover: null } }, 'id'), null)
})

// T6's data half: the label is counts, so it does not grow with the trail.
test('T6 the collapsed label is counts and a duration, not a list', () => {
  const trail: TrailEvent[] = [
    { kind: 'thought', id: '1', text: 'a' },
    { kind: 'tool', id: '2', name: 'memory_search', args: {} },
    { kind: 'tool', id: '3', name: 'fetch', args: {} },
    { kind: 'python', id: '4', intent: 'x', code: 'x', report: null },
  ]

  assert.equal(trailSummary(trail, 4200), 'Reasoning · 1 thought, 2 tools, 1 python run · 4.2s')
  assert.equal(trailSummary(trail), 'Reasoning · 1 thought, 2 tools, 1 python run')
})
