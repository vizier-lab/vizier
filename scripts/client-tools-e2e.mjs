#!/usr/bin/env node
// End-to-end check of the client tools protocol (specs/019-client-tools/quickstart.md §1–§8)
// against a running vizier with an agent on the offline `dummyplug` provider.
//
//   VIZIER_URL=ws://localhost:9999 VIZIER_TOKEN=<jwt> AGENT=plug node scripts/client-tools-e2e.mjs
//
// Dependency-free: Node ≥ 22 has a global WebSocket and fetch. Exits non-zero with the name of
// the first check that fails.

const BASE = (process.env.VIZIER_URL ?? 'ws://localhost:9999').replace(/\/$/, '')
const TOKEN = process.env.VIZIER_TOKEN
const AGENT = process.env.AGENT ?? 'plug'
const USER = process.env.VIZIER_USER ?? 'e2e'
const CHANNEL = 'e2e'
const HTTP = BASE.replace(/^ws/, 'http')

if (!TOKEN) {
  console.error('VIZIER_TOKEN is required')
  process.exit(2)
}

let current = 'setup'
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${current} › ${name}${detail === undefined ? '' : `\n  ${typeof detail === 'string' ? detail : JSON.stringify(detail).slice(0, 2000)}`}`)
    process.exit(1)
  }
  console.log(`  ok ${name}`)
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

function connect(topic) {
  const url = `${BASE}/api/v1/agents/${AGENT}/channel/${CHANNEL}/topic/${topic}/chat?token=${TOKEN}`
  const ws = new WebSocket(url)
  const frames = []
  const waiters = []
  ws.addEventListener('message', (event) => {
    let frame
    try {
      frame = JSON.parse(event.data)
    } catch {
      return
    }
    frames.push(frame)
    for (const waiter of [...waiters]) {
      if (waiter.predicate(frame)) {
        waiters.splice(waiters.indexOf(waiter), 1)
        waiter.resolve(frame)
      }
    }
  })
  const opened = new Promise((resolve, reject) => {
    ws.addEventListener('open', resolve, { once: true })
    ws.addEventListener('error', reject, { once: true })
  })
  return {
    topic,
    opened,
    frames,
    send(frame) {
      ws.send(JSON.stringify(frame))
    },
    /** The first frame from `from` on (an index into `frames`) matching `predicate`. */
    next(predicate, timeoutMs = 10000, from = 0) {
      const seen = frames.slice(from).find(predicate)
      if (seen) return Promise.resolve(seen)
      return new Promise((resolve, reject) => {
        const waiter = { predicate, resolve }
        waiters.push(waiter)
        setTimeout(() => {
          const at = waiters.indexOf(waiter)
          if (at >= 0) {
            waiters.splice(at, 1)
            reject(new Error(`no matching frame within ${timeoutMs} ms`))
          }
        }, timeoutMs)
      })
    },
    /** Whether a frame matching `predicate` arrives within `ms`. */
    async quiet(predicate, ms, from = 0) {
      await sleep(ms)
      return !frames.slice(from).some(predicate)
    },
    close() {
      ws.close()
    },
  }
}

const request = (content, extra = {}) => ({
  timestamp: new Date().toISOString(),
  user: USER,
  content,
  metadata: {},
  ...extra,
})
const runTool = (tool, args) => ({ chat: JSON.stringify({ tool, arguments: args }) })
const python = (code, intent = 'e2e') => runTool('execute_python', { code, intent })

const pickColor = (extra = {}) => ({
  name: 'pick_color',
  category: 'palette',
  description: 'Ask the person to pick a colour. Returns {"hex": "#rrggbb"}.',
  input_schema: { type: 'object', properties: { prompt: { type: 'string' } }, required: ['prompt'] },
  ...extra,
})
const declared = (tools) => ({
  client_tools: tools,
  client_categories: [{ name: 'palette', description: 'Colour pickers shown to the person' }],
})

const kind = (frame, key) => frame && Object.prototype.hasOwnProperty.call(frame, key)
const reply = (frame) => frame?.content?.message?.content
const isReply = (frame) => typeof reply(frame) === 'string'
const isError = (code) => (frame) => kind(frame, 'client_error') && frame.client_error.code === code

async function history(topic) {
  const res = await fetch(`${HTTP}/api/v1/agents/${AGENT}/channel/${CHANNEL}/topic/${topic}/history?limit=500`, {
    headers: { authorization: `Bearer ${TOKEN}` },
  })
  const body = await res.json()
  return body.data ?? []
}
const entryKind = (entry) => Object.keys(entry.content)[0]

/** Wait until `predicate(history)` holds, polling. */
async function historyUntil(topic, predicate, timeoutMs = 10000) {
  const until = Date.now() + timeoutMs
  for (;;) {
    const entries = await history(topic)
    if (predicate(entries)) return entries
    if (Date.now() > until) return entries
    await sleep(250)
  }
}

/** Send a request that makes a client tool call, and return the direct call frame. */
async function triggerCall(client, code, tools = [pickColor()]) {
  const from = client.frames.length
  client.send(request(python(code), declared(tools)))
  return (await client.next((f) => kind(f, 'client_tool_call') && f.client_tool_call.answer === true, 10000, from))
    .client_tool_call
}

const fresh = () => crypto.randomUUID()
const main = fresh()

// §1 ---------------------------------------------------------------------------------------
current = '§1 invalid declaration'
{
  const topic = fresh()
  const c = connect(topic)
  await c.opened
  c.send(request({ chat: 'tools' }, { client_tools: [{ name: 'Bad-Name', description: 'x', input_schema: { type: 'object' } }] }))
  const err = await c.next((f) => kind(f, 'client_error'))
  check('refused with invalid_client_tools', err.client_error.code === 'invalid_client_tools', err)
  check('names the rule', err.client_error.rule === 'name_pattern', err)
  check('no turn started', await c.quiet(isReply, 1500))
  const entries = await history(topic)
  check('no request saved', !entries.some((e) => entryKind(e) === 'Request'), entries)
  c.close()
}

// §2 ---------------------------------------------------------------------------------------
current = '§2 resolution and discovery'
const A = connect(main)
await A.opened
{
  const echo = { name: 'echo', category: 'memory', description: 'Echo it back', input_schema: { type: 'object' } }
  const code = "[list_tools(category='palette'), list_tools(category='memory_client'), describe_tool('palette_pick_color')['description']]"
  const resolvedFrames = []
  for (let i = 0; i < 2; i++) {
    const from = A.frames.length
    A.send(request(python(code), declared([pickColor(), echo])))
    const resolved = await A.next((f) => kind(f, 'client_tools_resolved'), 10000, from)
    resolvedFrames.push(resolved)
    const r = await A.next(isReply, 15000, from)
    if (i === 0) {
      const tools = resolved.client_tools_resolved.tools
      check('protocol 1', resolved.client_tools_resolved.protocol === 1, resolved)
      check('palette_pick_color in palette', tools.some((t) => t.name === 'palette_pick_color' && t.category_name === 'palette'), tools)
      check('memory renamed memory_client', tools.some((t) => t.name === 'memory_client_echo' && t.category_name === 'memory_client'), tools)
      check('resolved arrives before the reply', A.frames.indexOf(resolved) < A.frames.indexOf(r))
      check('both functions listed', reply(r).includes('palette_pick_color') && reply(r).includes('memory_client_echo'), reply(r))
      check('description labelled', reply(r).includes("(Provided by the person's client.)"), reply(r))
    }
  }
  check('same declaration resolves the same', JSON.stringify(resolvedFrames[0]) === JSON.stringify(resolvedFrames[1]))
}

// §3 ---------------------------------------------------------------------------------------
current = '§3 call round trip'
let callId
{
  const from = A.frames.length
  const call = await triggerCall(A, "palette_pick_color(prompt='header')")
  callId = call.call_id
  check('tool is the declared name', call.tool === 'pick_color' && call.category === 'palette', call)
  check('final name', call.name === 'palette_pick_color', call)
  check('arguments arrive', call.arguments.prompt === 'header', call)
  A.send({ client_tool_result: { call_id: callId, ok: { hex: '#3366ff' } } })
  const outcome = await A.next((f) => kind(f, 'client_tool_outcome') && f.client_tool_outcome.call_id === callId, 10000, from)
  check('outcome ok', outcome.client_tool_outcome.outcome.status === 'ok', outcome)
  const r = await A.next(isReply, 15000, from)
  check('result reaches the script', reply(r).includes('#3366ff'), reply(r))

  const entries = await historyUntil(main, (es) => es.some((e) => e.uid === callId))
  const at = entries.findIndex((e) => e.uid === callId)
  const entry = entries[at]
  check('call saved under its id', entry && entryKind(entry) === 'ClientToolCall', entries.map(entryKind))
  check('saved with outcome ok', entry.content.ClientToolCall.outcome?.status === 'ok', entry)
  const before = entries.slice(0, at).map(entryKind)
  const after = entries.slice(at + 1).map(entryKind)
  check('after its Request', before.lastIndexOf('Request') >= 0 && !before.slice(before.lastIndexOf('Request')).includes('Response'))
  check('before its Response', after.includes('Response'))
  const report = entries
    .slice(at)
    .find((e) => entryKind(e) === 'ToolResult')
  check('report keeps a reference, not the arguments', report && report.content.ToolResult.content.includes(`"client_call":"${callId}"`), report)
}

// §4 ---------------------------------------------------------------------------------------
current = '§4 error, disconnect, timeout, not pending'
{
  // 1. An error answer.
  let from = A.frames.length
  const errored = await triggerCall(A, "palette_pick_color(prompt='x')")
  A.send({ client_tool_result: { call_id: errored.call_id, error: 'closed' } })
  let r = await A.next(isReply, 15000, from)
  check('error reaches the script', reply(r).includes('closed') && reply(r).includes('Error'), reply(r))
  const o = await A.next((f) => kind(f, 'client_tool_outcome') && f.client_tool_outcome.call_id === errored.call_id, 5000, from)
  check('outcome error', o.client_tool_outcome.outcome.status === 'error', o)

  // 2. A disconnect.
  const topic = fresh()
  const D = connect(topic)
  await D.opened
  const code = "try:\n    r = palette_pick_color(prompt='x')\nexcept Exception as e:\n    r = str(e)\nr"
  const call = await triggerCall(D, code)
  const closedAt = Date.now()
  D.close()
  const entries = await historyUntil(
    topic,
    (es) => es.some((e) => entryKind(e) === 'Response' && JSON.stringify(e).includes('client_disconnected')),
    8000,
  )
  const response = entries.find((e) => entryKind(e) === 'Response' && JSON.stringify(e).includes('client_disconnected'))
  check('turn completes with client_disconnected', !!response, entries.map(entryKind))
  check('within 5 s', Date.now() - closedAt < 5000 + 1000)
  const saved = entries.find((e) => e.uid === call.call_id)
  check('outcome disconnected', saved?.content.ClientToolCall.outcome?.status === 'disconnected', saved)

  // 3. A timeout.
  from = A.frames.length
  const timed = await triggerCall(A, "try:\n    r = palette_pick_color(prompt='x')\nexcept Exception as e:\n    r = str(e)\nr", [pickColor({ timeout_secs: 2 })])
  const started = Date.now()
  await A.next((f) => kind(f, 'client_tool_cancelled') && f.client_tool_cancelled.call_id === timed.call_id, 6000, from)
  const waited = Date.now() - started
  check('cancelled after about 2 s', waited > 1500 && waited < 4000, waited)
  r = await A.next(isReply, 10000, from)
  check('script sees timeout', reply(r).includes('timeout'), reply(r))
  const t = await A.next((f) => kind(f, 'client_tool_outcome') && f.client_tool_outcome.call_id === timed.call_id, 5000, from)
  check('outcome timeout', t.client_tool_outcome.outcome.status === 'timeout', t)

  // 4. An answer for nothing.
  from = A.frames.length
  A.send({ client_tool_result: { call_id: 'made-up', ok: 1 } })
  const np = await A.next(isError('not_pending'), 5000, from)
  check('made-up id is not pending', np.client_error.call_id === 'made-up', np)
}

// §5 ---------------------------------------------------------------------------------------
current = '§5 only the declaring connection answers'
const B = connect(main)
await B.opened
{
  const fromA = A.frames.length
  const fromB = B.frames.length
  const call = await triggerCall(A, "palette_pick_color(prompt='two tabs')")
  const copy = await B.next((f) => kind(f, 'client_tool_call') && f.client_tool_call.call_id === call.call_id, 5000, fromB)
  check('B sees the call without answering it', copy.client_tool_call.answer === false, copy)
  check('A got no display copy', A.frames.slice(fromA).filter((f) => kind(f, 'client_tool_call') && f.client_tool_call.call_id === call.call_id).length === 1)
  B.send({ client_tool_result: { call_id: call.call_id, ok: { hex: '#000000' } } })
  await B.next(isError('not_pending'), 5000, fromB)
  check("B's answer is not pending", true)
  A.send({ client_tool_result: { call_id: call.call_id, ok: { hex: '#ffffff' } } })
  const r = await A.next(isReply, 15000, fromA)
  check("A's answer completes the call", reply(r).includes('#ffffff') && !reply(r).includes('#000000'), reply(r))
}

// §6 ---------------------------------------------------------------------------------------
current = '§6 client events'
{
  let from = A.frames.length
  A.send(request({ client_event: { text: 'tools', source_call_id: callId } }))
  let r = await A.next(isReply, 15000, from)
  check('event starts a turn', reply(r).includes('Available tools'), reply(r))

  from = A.frames.length
  A.send(request({ client_event: { text: 'Submitted filters', data: { x: 1 }, source_call_id: callId } }))
  await A.next(isReply, 15000, from)
  let entries = await history(main)
  const withData = entries.filter((e) => entryKind(e) === 'Request' && e.content.Request.content.client_event).at(-1)
  check('stored with its data and source', withData?.content.Request.content.client_event.data?.x === 1 &&
    withData.content.Request.content.client_event.source_call_id === callId, withData)

  // 3. A made-up source is dropped, the event still delivered.
  from = A.frames.length
  A.send(request({ client_event: { text: 'hello', source_call_id: 'made-up' } }))
  await A.next(isReply, 15000, from)
  entries = await history(main)
  const dropped = entries.filter((e) => entryKind(e) === 'Request' && e.content.Request.content.client_event).at(-1)
  check('made-up source dropped', dropped.content.Request.content.client_event.text === 'hello' &&
    dropped.content.Request.content.client_event.source_call_id === undefined, dropped)

  // 2. The rate limit, on a topic of its own so the count is exact.
  const R = connect(fresh())
  await R.opened
  for (let i = 0; i < 11; i++) R.send(request({ client_event: { text: `burst ${i}` } }))
  const limited = await R.next(isError('rate_limited'), 5000)
  check('the 11th is refused', !!limited, limited)
  check('only one refusal', R.frames.filter(isError('rate_limited')).length === 1)
  R.close()
}

// §7 ---------------------------------------------------------------------------------------
current = '§7 saved state'
{
  const requestsBefore = (await history(main)).filter((e) => entryKind(e) === 'Request').length
  let fromA = A.frames.length
  let fromB = B.frames.length
  A.send({ client_call_state: { call_id: callId, state: { n: 3 } } })
  const echoA = await A.next((f) => kind(f, 'client_call_state'), 5000, fromA)
  const echoB = await B.next((f) => kind(f, 'client_call_state'), 5000, fromB)
  check('saver gets the echo', echoA.client_call_state.state.n === 3, echoA)
  check('other tab is told', echoB.client_call_state.state.n === 3 && echoB.client_call_state.call_id === callId, echoB)
  let entries = await history(main)
  check('history carries the state', entries.find((e) => e.uid === callId)?.content.ClientToolCall.state?.n === 3)

  await sleep(1100)
  fromA = A.frames.length
  for (const n of [4, 5, 6]) A.send({ client_call_state: { call_id: callId, state: { n } } })
  await A.next(isError('rate_limited'), 5000, fromA)
  check('third save in a second refused', true)
  await sleep(300)
  entries = await history(main)
  check('stored state is the second', entries.find((e) => e.uid === callId)?.content.ClientToolCall.state?.n === 5,
    entries.find((e) => e.uid === callId))

  fromA = A.frames.length
  A.send({ client_call_state: { call_id: callId, state: { blob: 'x'.repeat(70 * 1024) } } })
  await A.next(isError('state_too_large'), 5000, fromA)
  check('70 KB refused', true)

  const requestUid = entries.find((e) => entryKind(e) === 'Request').uid
  fromA = A.frames.length
  A.send({ client_call_state: { call_id: requestUid, state: 1 } })
  await A.next(isError('not_found'), 5000, fromA)
  check('a Request uid is not found', true)

  await sleep(500)
  entries = await history(main)
  check('state unchanged by refusals', entries.find((e) => e.uid === callId)?.content.ClientToolCall.state?.n === 5)
  check('no new requests', entries.filter((e) => entryKind(e) === 'Request').length === requestsBefore)
}

// §8 ---------------------------------------------------------------------------------------
current = '§8 turns without a client'
{
  const topic = fresh()
  const N = connect(topic)
  await N.opened
  let from = N.frames.length
  N.send(request(python("list_tools(category='palette')")))
  const r = await N.next(isReply, 15000, from)
  check('unknown category without a declaration', /palette/.test(reply(r)) && /Error/.test(reply(r)), reply(r))
  check('nothing resolved', !N.frames.some((f) => kind(f, 'client_tools_resolved')))

  // A background job launched from a turn that declared tools reports in a turn of its own,
  // which has none.
  from = N.frames.length
  N.send(request(python("paralel_subtasks(tasks=[{'prompt': 'hello'}])"), declared([pickColor()])))
  await N.next((f) => kind(f, 'client_tools_resolved'), 10000, from)
  await N.next(isReply, 15000, from)
  const resolvedBefore = N.frames.filter((f) => kind(f, 'client_tools_resolved')).length
  const entries = await historyUntil(topic, (es) => {
    const at = es.findIndex((e) => entryKind(e) === 'Request' && e.content.Request.content.background_report)
    return at >= 0 && es.slice(at).some((e) => entryKind(e) === 'Response')
  }, 30000)
  const report = entries.find((e) => entryKind(e) === 'Request' && e.content.Request.content.background_report)
  check('report turn ran', !!report, entries.map(entryKind))
  check('report turn resolved no client tools', N.frames.filter((f) => kind(f, 'client_tools_resolved')).length === resolvedBefore)
  N.close()
}

A.close()
B.close()
console.log('\nall checks passed')
process.exit(0)
