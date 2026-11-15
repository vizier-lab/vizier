#!/usr/bin/env node
// End-to-end check of steering a busy turn (specs/062-steer-busy-turn/quickstart.md E1–E6)
// against a running vizier with an agent on the offline `dummyplug` provider.
//
//   dummyplug:  cd dummyplug && node src/server.mjs        (listens on :9998)
//   vizier:     just run                                   (or any server on VIZIER_URL)
//   this:       VIZIER_URL=ws://localhost:9999 VIZIER_TOKEN=<jwt> AGENT=plug node scripts/steer-e2e.mjs
//
// Needs dummyplug's `delay_ms` and §8 "Read mid-turn" (contracts/dummyplug-steer.md). E6 (a
// restart mid-turn) runs only with STEER_E2E_RESTART=1, since it restarts the agent.
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
    console.error(`FAIL ${current} › ${name}${detail === undefined ? '' : `\n  ${typeof detail === 'string' ? detail : JSON.stringify(detail).slice(0, 3000)}`}`)
    process.exit(1)
  }
  console.log(`  ok ${name}`)
}
const section = (name) => {
  current = name
  console.log(name)
}
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

function connect(topic) {
  const ws = new WebSocket(`${BASE}/api/v1/agents/${AGENT}/channel/${CHANNEL}/topic/${topic}/chat?token=${TOKEN}`)
  const frames = []
  ws.addEventListener('message', (event) => {
    try {
      frames.push(JSON.parse(event.data))
    } catch {}
  })
  const opened = new Promise((resolve, reject) => {
    ws.addEventListener('open', resolve, { once: true })
    ws.addEventListener('error', reject, { once: true })
  })
  return {
    opened,
    frames,
    send: (frame) => ws.send(JSON.stringify(frame)),
    close: () => ws.close(),
  }
}

const request = (text, extra = {}) => ({ timestamp: new Date().toISOString(), user: USER, content: { chat: text }, metadata: {}, ...extra })
/** A run request dummyplug answers with a tool call after `delayMs`: the window to steer in. */
const slowTool = (delayMs) => JSON.stringify({ tool: 'execute_python', arguments: { intent: 'e2e', code: '1 + 1' }, delay_ms: delayMs })
/** A final answer after `delayMs`, with no tool call: a busy turn with no round boundary. */
const slowAnswer = (say, delayMs) => JSON.stringify({ say, delay_ms: delayMs })

const reply = (frame) => frame?.content?.message?.content
const replies = (client) => client.frames.map(reply).filter((text) => typeof text === 'string')

async function api(method, path, body) {
  const res = await fetch(`${HTTP}/api/v1${path}`, {
    method,
    headers: { 'content-type': 'application/json', authorization: `Bearer ${TOKEN}` },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  return { status: res.status, body: await res.json().catch(() => ({})) }
}

/** The topic's history, oldest first. */
async function history(topic) {
  const { body } = await api('GET', `/agents/${AGENT}/channel/${CHANNEL}/topic/${topic}/history?limit=500`)
  const entries = body.data ?? []
  return [...entries].sort((a, b) => (a.seq ?? 0) - (b.seq ?? 0))
}
const kindOf = (entry) => Object.keys(entry.content)[0]
const requests = (entries) => entries.filter((e) => kindOf(e) === 'Request').map((e) => e.content.Request)
const chatText = (r) => r?.content?.chat
const steeredTexts = (entries) => requests(entries).filter((r) => r.mid_turn).map(chatText)

async function waitFor(predicate, timeoutMs = 15000) {
  const until = Date.now() + timeoutMs
  for (;;) {
    if (await predicate()) return true
    if (Date.now() > until) return false
    await sleep(200)
  }
}

async function topic(name) {
  const id = `${name}-${Date.now().toString(36)}`
  const client = connect(id)
  await client.opened
  return { id, client }
}

// E1 — a message steers the running turn (Story 1, FR-001/002/005).
section('E1 a message sent while busy reaches the next step')
{
  const { id, client } = await topic('steer')
  client.send(request(slowTool(2000)))
  await sleep(600)
  client.send(request('keep the old token format'))
  check('the turn answered', await waitFor(() => replies(client).length >= 1))
  await sleep(1500)
  check('one reply for both messages', replies(client).length === 1, replies(client))
  check('the reply read the message mid-turn', replies(client)[0] === '**Read mid-turn**:\nkeep the old token format', replies(client))
  const entries = await history(id)
  const kinds = entries.map(kindOf)
  check('history: request, call, result, mid-turn request, response', JSON.stringify(kinds) === JSON.stringify(['Request', 'ToolCall', 'ToolResult', 'Request', 'Response']), kinds)
  check('the read message is marked mid_turn', JSON.stringify(steeredTexts(entries)) === JSON.stringify(['keep the old token format']), requests(entries))
  client.close()
}

// E2 — several messages at one boundary (Story 3).
section('E2 several messages arrive together, in order')
{
  const { id, client } = await topic('burst')
  client.send(request(slowTool(2000)))
  await sleep(500)
  for (const text of ['one', 'two', 'three']) {
    client.send(request(text))
    await sleep(100)
  }
  check('the turn answered', await waitFor(() => replies(client).length >= 1))
  await sleep(1500)
  check('one reply', replies(client).length === 1, replies(client))
  check('all three read in order', replies(client)[0] === '**Read mid-turn**:\none\ntwo\nthree', replies(client))
  check('three mid-turn entries in order', JSON.stringify(steeredTexts(await history(id))) === JSON.stringify(['one', 'two', 'three']))
  client.close()
}

// E3 — `/queue` and the `queue` field keep today's behaviour (Story 5).
section('E3 /queue and queue: true run as a new turn')
{
  const { id, client } = await topic('queue')
  client.send(request(slowTool(2000)))
  await sleep(500)
  client.send(request('/queue lorem please'))
  client.send(request('flagged', { queue: true }))
  check('three turns answered', await waitFor(() => replies(client).length >= 3, 20000), replies(client))
  const [first] = replies(client)
  check('the first turn read nothing mid-turn', first.startsWith('**Tool result**'), first)
  const entries = await history(id)
  check('nothing was read mid-turn', steeredTexts(entries).length === 0, requests(entries))
  const texts = requests(entries).map(chatText)
  check('the queued messages ran as their own turns, without /queue', texts.includes('lorem please') && texts.includes('flagged') && !texts.some((t) => t?.startsWith('/queue')), texts)

  const before = (await history(id)).length
  client.send(request('/queue'))
  check('a bare /queue is answered with its usage', await waitFor(() => replies(client).some((t) => t.startsWith('Usage: `/queue'))))
  await sleep(500)
  check('a bare /queue saves nothing', (await history(id)).length === before)
  client.close()
}

// E4 — no boundary left: fall back to the queue (Story 2, FR-004).
section('E4 a turn with no further step leaves the message for its own turn')
{
  const { id, client } = await topic('fallback')
  client.send(request(slowAnswer('done', 2000)))
  await sleep(500)
  client.send(request('second'))
  check('two turns answered', await waitFor(() => replies(client).length >= 2, 20000), replies(client))
  check('the first answered alone', replies(client)[0] === 'done', replies(client))
  const entries = await history(id)
  check('second ran as its own turn, not mid-turn', steeredTexts(entries).length === 0 && requests(entries).some((r) => chatText(r) === 'second' && !r.mid_turn), requests(entries))
  client.close()
}

// E5 — abort while a message waits (FR-009).
section('E5 /abort drops a waiting message')
{
  const { id, client } = await topic('abort')
  client.send(request(slowTool(2500)))
  await sleep(500)
  client.send(request('wait'))
  await sleep(200)
  client.send({ ...request(''), content: { command: 'abort' } })
  await sleep(3500)
  const entries = await history(id)
  check('the waiting message was never read or run', !requests(entries).some((r) => chatText(r) === 'wait'), requests(entries))
}

// E6 — restart mid-turn (FR-003, SC-002). Opt-in: it restarts the agent.
if (process.env.STEER_E2E_RESTART === '1') {
  section('E6 a restart mid-turn reads the message exactly once')
  const { id, client } = await topic('restart')
  client.send(request(slowTool(5000)))
  await sleep(800)
  client.send(request('after restart'))
  await sleep(300)
  const agent = await api('GET', `/agents/${AGENT}`)
  check('read the agent config', agent.status === 200, agent.body)
  const restarted = await api('PUT', `/agents/${AGENT}`, agent.body.data)
  check('restarted the agent', restarted.status < 300, restarted.body)
  check('the resumed turn finished', await waitFor(async () => (await history(id)).some((e) => kindOf(e) === 'Response'), 30000))
  await sleep(2000)
  const entries = await history(id)
  const copies = requests(entries).filter((r) => chatText(r) === 'after restart')
  check('the message is in history exactly once', copies.length === 1, requests(entries))
  check('and answered once', entries.filter((e) => kindOf(e) === 'Response').length === 1, entries.map(kindOf))
  client.close()
}

console.log('all steering checks passed')
process.exit(0)
