// Unit tests for the pure trail producers. Run with the Node test runner, which strips the
// types natively, so this needs no test framework and no new dependency:
//
//   cd webui && npx vitest run app/lib/trail.test.ts
//
// The guarantee numbers are from
// `specs/010-webui-reasoning-display/contracts/trail-model.md` §3.

import { test } from 'vitest'
import assert from 'node:assert/strict'

import {
  appendClientCall,
  appendLiveEvent,
  applyClientOutcome,
  applyClientState,
  blockDurationMs,
  groupHistory,
  liveEvent,
  mergeEntries,
  openTurn,
  trailSummary,
  turnEvents,
} from './trail.ts'
import type { ChatMessage, TrailEvent, Turn, TurnBlock } from '../api/frames.ts'

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

// Prose between rounds reads as a thought on the spine, so the shape helpers name it so.
const kinds = (trail: TrailEvent[]) => trail.map((e) => (e.kind === 'narration' ? 'thought' : e.kind))

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
  assert.deepEqual(kinds(turnEvents(turns[0])), ['thought', 'tool', 'python', 'tool'])
  assert.equal((turnEvents(turns[0])[1] as { name: string }).name, 'memory_search')
  assert.equal((turnEvents(turns[0])[3] as { name: string }).name, 'fetch')
})

// T2 — a turn with nothing on the way to its answer has an empty trail, so the renderer has
// no trail element to draw.
test('T2 an empty trail yields no trail element', () => {
  const turns = groupHistory([request('hello'), response('hi')])

  assert.equal(turns.length, 1)
  assert.deepEqual(turns[0].blocks, [])
})

// T3 — adjacent reasoning reads as one block, not as a list of fragments.
test('T3 adjacent thoughts merge', () => {
  const turns = groupHistory([
    request('why?'),
    think('first'),
    think('second'),
    tool('memory_search', { query: 'x' }),
    think('third'),
    response('because'),
  ])

  assert.deepEqual(kinds(turnEvents(turns[0])), ['thought', 'tool', 'thought'])
  assert.equal((turnEvents(turns[0])[0] as { text: string }).text, 'first\n\nsecond')
})

// ---------------------------------------------------------------------------------------------
// Mid-turn messages (specs/053-workspace-channel/contracts/turn-rendering.md)
// ---------------------------------------------------------------------------------------------

const say = (text: string, timestamp?: string) => ({ ...entry({ AssistantMessage: text }), timestamp })
const at = (message: ChatMessage, timestamp: string) => ({ ...message, timestamp })
const shape = (blocks: TurnBlock[]) => blocks.map((b) => (b.kind === 'trail' ? kinds(b.events).join('+') : `"${b.text}"`))

test('text between two rounds stays in the one trail, where it was written', () => {
  const turns = groupHistory([
    request('go'),
    tool('memory_search'),
    say('The log points at the deploy.'),
    tool('fetch'),
    response('done'),
  ])

  assert.equal(turns.length, 1)
  assert.deepEqual(shape(turns[0].blocks), ['tool+thought+tool'])
  const said = turnEvents(turns[0])[1]
  assert.equal(said.kind === 'narration' && said.text, 'The log points at the deploy.')
  assert.ok(turns[0].outcome)
})

test('a turn never shows more than one trail, however much it says on the way', () => {
  const turns = groupHistory([
    request('go'),
    say('Checking first.'),
    tool('fetch'),
    say('Now the log.'),
    tool('memory_search'),
    say('Almost there.'),
    response('done'),
  ])
  assert.deepEqual(shape(turns[0].blocks), ['thought+tool+thought+tool+thought'])
})

test('text first opens the trail rather than sitting above it', () => {
  const turns = groupHistory([request('go'), say('Checking first.'), tool('fetch'), response('done')])
  assert.deepEqual(shape(turns[0].blocks), ['thought+tool'])
})

test('text with no call at all still makes a trail of its own', () => {
  const turns = groupHistory([request('go'), say('one'), say('two'), response('done')])
  assert.deepEqual(shape(turns[0].blocks), ['thought+thought'])
})

test('an empty message makes no step', () => {
  const turns = groupHistory([request('go'), tool('fetch'), say(''), tool('fetch'), response('done')])
  assert.deepEqual(shape(turns[0].blocks), ['tool+tool'])
})

test('a report pairs with its call across the text written while it ran', () => {
  const report = { ok: true, result: 1, stdout: '', tool_calls: [], duration_ms: 3 }
  const turns = groupHistory([
    request('go'),
    python('c1', { intent: 'run', code: '1' }),
    say('A message written while the script ran.'),
    toolResult('c1', JSON.stringify(report)),
    response('done'),
  ])
  const run = turnEvents(turns[0])[0]
  assert.equal(run.kind === 'python' && run.report?.result, 1)
  assert.deepEqual(shape(turns[0].blocks), ['python+thought'])
})

test('a stopped turn keeps its work and marks the cut-off call', () => {
  const turns = groupHistory([
    request('go'),
    tool('memory_search'),
    say('Looking further.'),
    python('c1', { intent: 'run', code: '1' }),
    toolResult('c1', 'Stopped: the person aborted the turn before this call finished.'),
    entry({ Command: 'abort' }),
  ])
  assert.equal(turns[0].aborted, true)
  assert.deepEqual(shape(turns[0].blocks), ['tool+thought+python'])
  const run = turnEvents(turns[0])[2]
  assert.equal(run.kind === 'python' && run.stopped, true)
})

test('the trail spans the whole turn, text included', () => {
  const turns = groupHistory([
    request('go'),
    at(python('c1', { code: '1' }), '2026-10-03T00:00:10Z'),
    at(toolResult('c1', '{}'), '2026-10-03T00:00:14Z'),
    at(say('half way'), '2026-10-03T00:00:15Z'),
    at(tool('fetch'), '2026-10-03T00:00:20Z'),
    at(tool('fetch'), '2026-10-03T00:00:21Z'),
    response('done'),
  ])
  const trails = turns[0].blocks.filter((b) => b.kind === 'trail')
  assert.equal(trails.length, 1)
  assert.equal(trails[0].kind === 'trail' && blockDurationMs(trails[0]), 11000)
})

test('old saved history with text between rounds folds into one trail, with no migration', () => {
  const saved = JSON.parse(
    JSON.stringify([
      request('go'),
      entry({ AssistantMessage: 'From before the change.' }),
      entry({ ToolCall: { call_id: 'old', name: 'fetch', arguments: {} } }),
      entry({ ToolResult: { call_id: 'old', content: 'ok' } }),
      response('done'),
    ])
  ) as ChatMessage[]
  assert.deepEqual(shape(groupHistory(saved)[0].blocks), ['thought+tool'])
})

test('a bare newline a model sends beside each call is not a step', () => {
  const turn = groupHistory([
    request('go'),
    entry({ AssistantMessage: '\n\n' }),
    entry({ ToolCall: { call_id: 'a', name: 'fetch', arguments: {} } }),
    entry({ ToolResult: { call_id: 'a', content: 'ok' } }),
    entry({ AssistantMessage: '\n\n' }),
    entry({ ToolCall: { call_id: 'b', name: 'fetch', arguments: {} } }),
    entry({ ToolResult: { call_id: 'b', content: 'ok' } }),
    response('done'),
  ])[0]
  assert.equal(turn.blocks.length, 1)
  assert.equal(turn.blocks[0].kind === 'trail' && turn.blocks[0].events.length, 2)
})

const endedBy = (text: string, by: string | null) =>
  entry({
    Response: {
      timestamp: '2026-10-03T00:00:01Z',
      content: { message: { content: text, stats: { ended_by_tool: by } as never } },
    },
  })

test('the text a turn-ending call wrote is drawn once, as the reply', () => {
  const turns = groupHistory([
    request('plan it'),
    tool('memory_search'),
    say('Here is the plan.'),
    python('c1', { code: 'propose_plan()' }),
    toolResult('c1', '{}'),
    endedBy('Here is the plan.', 'propose_plan'),
  ])
  assert.deepEqual(shape(turns[0].blocks), ['tool+thought+python'])
})

test('a genuine repeat in an ordinary turn is kept', () => {
  const turns = groupHistory([request('go'), say('same'), tool('fetch'), endedBy('same', null)])
  assert.deepEqual(shape(turns[0].blocks), ['thought+tool'])
})

// T11 — an agent-initiated turn (a scheduled task, a dream cycle) has no request. Its trail
// belongs to a turn rather than being dropped.
test('T11 an entry with no open turn yields a turn with request undefined', () => {
  const turns = groupHistory([think('unprompted'), response('done')])

  assert.equal(turns.length, 1)
  assert.equal(turns[0].request, undefined)
  assert.deepEqual(kinds(turnEvents(turns[0])), ['thought'])
  assert.ok(turns[0].outcome)
})

// T8 — one entry kind this version does not know about must not blank the conversation.
test('T8 an unrecognised entry kind is skipped without throwing', () => {
  const unknown = entry({ SomethingNew: { whatever: true } } as unknown as ChatMessage['content'])
  const turns = groupHistory([request('hi'), unknown, think('still here'), response('hi back')])

  assert.equal(turns.length, 1)
  assert.deepEqual(kinds(turnEvents(turns[0])), ['thought'])
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

  assert.deepEqual(kinds(turnEvents(turns[0])), ['thought'])
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

  const event = turnEvents(turns[0])[0]
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

  const event = turnEvents(turns[0])[0]
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
  assert.deepEqual(kinds(turnEvents(turns[0])), ['thought'])
  assert.deepEqual(kinds(turnEvents(turns[1])), ['thought'])
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

  assert.deepEqual(kinds(turnEvents(turns[0])), ['python'])
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

// A task run's history opens with a `Request` entry carrying `unattended` content. Rendering
// it through `groupHistory` is the whole reason reusing this function was attractive, so the
// turn has to open without adopting that entry — otherwise every run's trail gains a
// nameless empty bubble and the task's instruction appears twice on screen.
const unattendedRequest = (instruction: string) =>
  entry({
    Request: {
      timestamp: '2026-10-04T09:00:00Z',
      user: 'scheduler',
      content: { unattended: instruction },
    },
  })

test('an unattended request opens a turn without becoming its request', () => {
  const thought = think('checking the merged PRs')
  const call = tool('fetch', { url: 'https://example.invalid' })
  const outcome = response('Posted to #eng. 4 PRs merged since yesterday.')

  const turns = groupHistory([
    unattendedRequest('Summarise merged PRs and post to #eng'),
    thought,
    call,
    outcome,
  ])

  assert.equal(turns.length, 1)
  assert.equal(turns[0].request, undefined)
  // The trail and the outcome are intact — the turn boundary is kept, only the request
  // bubble is not adopted.
  assert.equal(turnEvents(turns[0]).length, 2)
  assert.equal(turnEvents(turns[0])[0].kind, 'thought')
  assert.equal(turnEvents(turns[0])[1].kind, 'tool')
  assert.equal(turns[0].outcome?.uid, outcome.uid)
  assert.equal(turns[0].anchorUid, outcome.uid)
})

test('an ordinary chat request is still adopted as its turn request', () => {
  const asked = request('how are the PRs looking?')
  const turns = groupHistory([asked, response('four merged since yesterday')])

  assert.equal(turns.length, 1)
  assert.equal(turns[0].request?.uid, asked.uid)
})

// Background jobs (saved before spec 058, #82): the call that launched a job is tagged with the job id its result quotes,
// so the job tray can jump back to it.
test('a launching call in history is tagged with the job id from its result', () => {
  const turns = groupHistory([
    entry({ Request: { timestamp: 't', user: 'u', content: { chat: 'go' } } }),
    entry({ ToolCall: { call_id: 'c1', name: 'paralel_subtasks', arguments: { tasks: [] } } }),
    entry({
      ToolResult: {
        call_id: 'c1',
        content: '"Started background batch b-7f3a9c with 2 tasks. Results will arrive later."',
      },
    }),
    entry({ ToolCall: { call_id: 'c2', name: 'fetch', arguments: {} } }),
    entry({ ToolResult: { call_id: 'c2', content: 'b-000000 is not a job here' } }),
  ])

  const tools = turnEvents(turns[0]).filter((event) => event.kind === 'tool')
  assert.equal(tools.length, 2)
  assert.equal(tools[0].kind === 'tool' && tools[0].jobId, 'b-7f3a9c')
  assert.equal(tools[1].kind === 'tool' && tools[1].jobId, undefined)
})

test('background_subagents is a job launch', () => {
  const turns = groupHistory([
    entry({ Request: { timestamp: 't', user: 'u', content: { chat: 'go' } } }),
    entry({ ToolCall: { call_id: 'c1', name: 'background_subagents', arguments: { tasks: [] } } }),
    entry({
      ToolResult: {
        call_id: 'c1',
        content: '"Started background batch b-7f3a9c with 2 tasks. Results will arrive later."',
      },
    }),
  ])

  const tools = turnEvents(turns[0]).filter((event) => event.kind === 'tool')
  assert.equal(tools.length, 1)
  assert.equal(tools[0].kind === 'tool' && tools[0].jobId, 'b-7f3a9c')
})

test('the retired paralel_subtasks name is still a job launch in saved history', () => {
  const turns = groupHistory([
    entry({ Request: { timestamp: 't', user: 'u', content: { chat: 'go' } } }),
    entry({ ToolCall: { call_id: 'c1', name: 'paralel_subtasks', arguments: { tasks: [] } } }),
    entry({
      ToolResult: {
        call_id: 'c1',
        content: '"Started background batch b-7f3a9c with 2 tasks. Results will arrive later."',
      },
    }),
  ])

  const tools = turnEvents(turns[0]).filter((event) => event.kind === 'tool')
  assert.equal(tools.length, 1)
  assert.equal(tools[0].kind === 'tool' && tools[0].jobId, 'b-7f3a9c')
})

test('parallel_subagents is an ordinary tool call', () => {
  const turns = groupHistory([
    entry({ Request: { timestamp: 't', user: 'u', content: { chat: 'go' } } }),
    entry({ ToolCall: { call_id: 'c1', name: 'parallel_subagents', arguments: { tasks: [] } } }),
    entry({ ToolResult: { call_id: 'c1', content: '{"results":[]}' } }),
  ])

  const tools = turnEvents(turns[0]).filter((event) => event.kind === 'tool')
  assert.equal(tools.length, 1)
  assert.equal(tools[0].kind === 'tool' && tools[0].jobId, undefined)
})

test('a live launch result tags the most recent launching call', () => {
  let trail: TrailEvent[] = []
  trail = appendLiveEvent(
    trail,
    { tool_choice: { name: 'delegate_agent', args: { agent_id: 'b', prompt: 'x' } } },
    'e1'
  )
  trail = appendLiveEvent(
    trail,
    { tool_response: { response: "Delegated to agent 'b' as background job b-91c2e0." } },
    'e2'
  )
  assert.equal(trail.length, 1)
  assert.equal(trail[0].kind === 'tool' && trail[0].jobId, 'b-91c2e0')
})

// ---------------------------------------------------------------------------------------------
// Client tool calls (specs/019-client-tools R11)
// ---------------------------------------------------------------------------------------------

const session = { agent_id: 'a', channel: 'http__someone__webui', topic: 'General' }

const clientCall = (uid: string, extra: Record<string, unknown> = {}): ChatMessage => ({
  uid,
  vizier_session: session,
  content: {
    ClientToolCall: {
      name: 'ui_render_interactive_ui',
      category: 'ui',
      tool: 'render_interactive_ui',
      arguments: { title: 'Hello', html: '<p>hi</p>' },
      ...extra,
    },
  },
})

const callFrame = (call_id: string) => ({
  call_id,
  name: 'ui_render_interactive_ui',
  category: 'ui',
  tool: 'render_interactive_ui',
  arguments: { title: 'Hello', html: '<p>hi</p>' },
  answer: true,
})

test('a client call lands in its turn, between request and outcome', () => {
  const turns = groupHistory([
    request('show me'),
    clientCall('call-1', { outcome: { status: 'ok', result: { status: 'rendered' } } }),
    python('p1', { intent: 'draw', code: 'ui_render_interactive_ui()' }),
    response('there it is'),
    request('next'),
    response('ok'),
  ])
  assert.equal(turns.length, 2)
  assert.deepEqual(turns[0].clientCalls.map((c) => c.callId), ['call-1'])
  assert.equal(turns[0].clientCalls[0].tool, 'render_interactive_ui')
  assert.equal(turns[0].clientCalls[0].outcome?.status, 'ok')
  // Not a trail event.
  assert.deepEqual(kinds(turnEvents(turns[0])), ['python'])
  assert.deepEqual(turns[1].clientCalls, [])
})

test('a turn that never finished keeps its calls', () => {
  const turns = groupHistory([request('show me'), clientCall('call-1', { outcome: { status: 'cancelled' } })])
  assert.equal(turns.length, 1)
  assert.equal(turns[0].outcome, undefined)
  assert.equal(turns[0].clientCalls[0].outcome?.status, 'cancelled')
})

test('live and reloaded turns give the same client calls', () => {
  const req = request('show me')
  let live: ChatMessage[] = [req]
  live = appendClientCall(live, callFrame('call-1'), session)
  // The display copy of the same call changes nothing.
  live = appendClientCall(live, { ...callFrame('call-1'), answer: false }, session)
  live = applyClientOutcome(live, { call_id: 'call-1', outcome: { status: 'ok', result: { ui_id: 'call-1' } } })
  live = applyClientState(live, { call_id: 'call-1', state: { n: 3 } })
  const reply = response('done')
  live = [...live, reply]

  const stored = [
    req,
    clientCall('call-1', { outcome: { status: 'ok', result: { ui_id: 'call-1' } }, state: { n: 3 } }),
    reply,
  ]
  assert.deepEqual(groupHistory(live)[0].clientCalls, groupHistory(stored)[0].clientCalls)
  assert.equal(groupHistory(live)[0].clientCalls[0].state && (groupHistory(live)[0].clientCalls[0].state as { n: number }).n, 3)
})

test('outcome and state updates apply by call id only', () => {
  const messages = [clientCall('a'), clientCall('b')]
  const next = applyClientState(messages, { call_id: 'b', state: 1 })
  assert.equal(next[0], messages[0])
  assert.equal(next[1].content.ClientToolCall?.state, 1)
  assert.equal(applyClientOutcome(messages, { call_id: 'nope', outcome: { status: 'timeout' } }), messages)
})

test('a call with no open turn opens one', () => {
  const turns = groupHistory([request('x'), response('y'), clientCall('c')])
  assert.equal(turns.length, 2)
  assert.deepEqual(turns[1].clientCalls.map((c) => c.callId), ['c'])
})

test('unknown entry kinds next to client calls still do not throw', () => {
  const odd = { ...entry({}), content: { Mystery: 1 } as unknown as ChatMessage['content'] }
  assert.doesNotThrow(() => groupHistory([request('x'), odd, clientCall('c'), response('y')]))
})

// specs/023-persist-turn-activity: the running trail is built from history, merged by uid.

const stepCall = (callId: string) =>
  entry({ ToolCall: { call_id: callId, name: 'execute_python', arguments: { intent: 'x', code: 'y' } } })
const stepResult = (callId: string) => entry({ ToolResult: { call_id: callId, content: 'done' } })

test('merging overlapping entries shows each step once, in stored order', () => {
  const a = request('go')
  const b = stepCall('c1')
  const c = stepResult('c1')
  const merged = mergeEntries([a, b], [b, c])
  assert.deepEqual(merged.map((m) => m.uid), [a.uid, b.uid, c.uid])
})

test('merging with a gap appends the new steps after the held ones', () => {
  const a = request('go')
  const b = stepCall('c1')
  const c = stepCall('c2')
  const merged = mergeEntries([a], [b, c])
  assert.deepEqual(merged.map((m) => m.uid), [a.uid, b.uid, c.uid])
})

test('an incoming entry replaces the one with the same uid', () => {
  const a = request('go')
  const replaced = { ...a, reactions: [] }
  const merged = mergeEntries([a], [replaced])
  assert.equal(merged.length, 1)
  assert.equal(merged[0], replaced)
})

test('a turn with an unanswered tool call is the open turn', () => {
  const turns = groupHistory([request('go'), stepCall('c1')])
  assert.equal(turnEvents(openTurn(turns) as Turn).length, 1)
  assert.equal(openTurn(groupHistory([request('go'), stepCall('c1'), stepResult('c1'), response('ok')])), undefined)
})

test('an aborted turn has no outcome and is marked aborted, not open', () => {
  const turns = groupHistory([request('go'), stepCall('c1'), stepResult('c1'), entry({ Command: 'abort' })])
  assert.equal(turns[0].aborted, true)
  assert.equal(openTurn(turns), undefined)
})

test('a turn ended by a turn-ending call keeps the call and its empty reply together', () => {
  const turns = groupHistory([
    request('chart it'),
    clientCall('call-1', { outcome: { status: 'ok', result: { status: 'rendered' } } }),
    response(''),
  ])
  assert.equal(turns.length, 1)
  assert.equal(turns[0].clientCalls.length, 1)
  assert.ok(turns[0].outcome)
  assert.equal(turns[0].anchorUid, turns[0].outcome?.uid)
})

test('a detached run reads as running in the background, from history and live', () => {
  const handle = { detached: true, id: 'd_k3x9a1b2', state: 'running', timeout_secs: 600 }
  const turns = groupHistory([
    request('slow'),
    python('c-d', { intent: 'run the tests', code: 'x', detached: true }),
    toolResult('c-d', JSON.stringify(handle)),
    response('started'),
  ])
  const event = turnEvents(turns[0])[0]
  assert.equal(event.kind, 'python')
  if (event.kind === 'python') {
    assert.equal(event.detachedId, 'd_k3x9a1b2')
    assert.equal(event.report, null)
  }

  const live = appendLiveEvent(
    appendLiveEvent([], { tool_choice: { name: 'execute_python', args: { intent: 'x', code: 'y' } } }, 'l1'),
    { tool_response: { response: handle } },
    'l2',
  )
  const run = live.find((e) => e.kind === 'python')
  assert.ok(run && run.kind === 'python' && run.detachedId === 'd_k3x9a1b2')
})

// Spec 058: a detached script's delivery is saved as a request, but mid-turn it is a step.
const delivery = (text: string) =>
  entry({ Request: { timestamp: '2026-10-03T00:00:00Z', user: 'a', content: { detached_delivery: text } } })
const DELIVERED = `[Background work finished]
d_aaaaaaaa  succeeded in 6s  — check vendor A
result: 3
d_bbbbbbbb  timed out in 10m 0s  — fetch the changelog
reason: the script's time limit (600s) was reached`

test('a mid-turn delivery is a step of its turn, which stays one turn', () => {
  const turns = groupHistory([
    request('check both'),
    python('p1', { intent: 'launch', code: '1' }),
    toolResult('p1', JSON.stringify({ detached: true, id: 'd_aaaaaaaa', state: 'running', timeout_secs: 600 })),
    delivery(DELIVERED),
    python('p2', { intent: 'compare', code: '2' }),
    toolResult('p2', JSON.stringify({ ok: true, result: 1, stdout: '', tool_calls: [], duration_ms: 3 })),
    response('done'),
  ])
  assert.equal(turns.length, 1)
  assert.equal(openTurn(turns), undefined)
  const events = turnEvents(turns[0])
  assert.deepEqual(kinds(events), ['python', 'delivery', 'delivery', 'python'])
  const first = events[1]
  assert.ok(first.kind === 'delivery')
  assert.equal(first.handle, 'd_aaaaaaaa')
  assert.equal(first.state, 'succeeded')
  assert.equal(first.took, '6s')
  assert.equal(first.intent, 'check vendor A')
  assert.equal(first.body, 'result: 3')
  const second = events[2]
  assert.ok(second.kind === 'delivery' && second.took === '10m 0s' && second.state === 'timed out')
})

// Spec 059: a worker's update rides the same delivery, after any scripts.
test('a mid-turn subagent update is a step of its turn, one per worker', () => {
  const turns = groupHistory([
    request('split it'),
    delivery(`${DELIVERED}

[Subagent update] s_k3x9a1b2 — task 2 of 3 "find the EU sheet"
state: asking
question: Which one?
reply with reply_subagent(id="s_k3x9a1b2", text=...)`),
    response('done'),
  ])
  assert.equal(turns.length, 1)
  const events = turnEvents(turns[0])
  assert.deepEqual(kinds(events), ['delivery', 'delivery', 'worker_update'])
  const update = events[2]
  assert.ok(update.kind === 'worker_update')
  assert.equal(update.handle, 's_k3x9a1b2')
  assert.equal(update.position, 2)
  assert.equal(update.state, 'asking')
  assert.equal(update.body, 'question: Which one?')
  assert.equal(events[1].kind === 'delivery' && events[1].body, "reason: the script's time limit (600s) was reached")
})

test('a delivery that wakes an idle agent starts its own turn', () => {
  const wake = delivery(DELIVERED)
  const turns = groupHistory([request('hi'), response('hello'), wake, python('p3', { intent: 'x', code: '3' }), response('noted')])
  assert.equal(turns.length, 2)
  assert.equal(turns[1].request?.uid, wake.uid)
  assert.deepEqual(kinds(turnEvents(turns[1])), ['python'])
})
