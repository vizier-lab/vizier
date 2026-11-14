#!/usr/bin/env node
// End-to-end check of subagents as temporary workers (specs/059-subagent-workers/quickstart.md)
// against a running vizier with an agent on the offline `dummyplug` provider and a local shell.
//
//   dummyplug:  cd dummyplug && node src/server.mjs        (listens on :9998)
//   vizier:     just run                                   (or any server on VIZIER_URL)
//   this:       VIZIER_URL=ws://localhost:9999 VIZIER_SETUP_USER=e2e node scripts/subagent-e2e.mjs
//
// A worker is a turn of the same agent, so dummyplug reads a worker's task text as its message:
// a task that is a dummyplug JSON request makes the worker run that tool. The restart check (§8)
// runs only when VIZIER_STOP and VIZIER_START are set: shell commands that stop vizier and start
// it again (detached). Dependency-free: Node ≥ 22. Exits non-zero at the first failed check.

import { execSync } from 'node:child_process'

const BASE = (process.env.VIZIER_URL ?? 'ws://localhost:9999').replace(/\/$/, '')
const HTTP = BASE.replace(/^ws/, 'http')
let TOKEN = process.env.VIZIER_TOKEN
const AGENT = process.env.AGENT ?? 'workers'
const USER = process.env.VIZIER_SETUP_USER ?? 'e2e'
const CHANNEL = 'e2e'

let current = 'setup'
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${current} › ${name}${detail === undefined ? '' : `\n  ${JSON.stringify(detail).slice(0, 3000)}`}`)
    process.exit(1)
  }
  console.log(`  ok ${name}`)
}
const section = (name) => {
  current = name
  console.log(name)
}
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const fresh = (name) => `${name}-${Date.now().toString(36)}`

async function api(method, path, body) {
  const res = await fetch(`${HTTP}/api/v1${path}`, {
    method,
    headers: { 'content-type': 'application/json', ...(TOKEN ? { authorization: `Bearer ${TOKEN}` } : {}) },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  const json = await res.json().catch(() => ({}))
  return { status: res.status, data: json.data, body: json }
}

/** A dummyplug request that runs `code` in `execute_python`. */
const py = (code) => JSON.stringify({ tool: 'execute_python', arguments: { intent: 'e2e', code } })
/** A Python string literal holding `text` (a JSON string literal is one). */
const lit = (text) => JSON.stringify(text)
const shellTask = (command) => py(`shell_exec(commands=${lit(command)})\n'done'`)
const QUICK = py("'quick'")
const slow = (secs) => shellTask(`sleep ${secs}`)

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
    send: (content) =>
      ws.send(JSON.stringify({ timestamp: new Date().toISOString(), user: USER, content, metadata: {} })),
    async until(predicate, timeoutMs = 60000) {
      const end = Date.now() + timeoutMs
      for (;;) {
        const found = frames.find(predicate)
        if (found) return found
        if (Date.now() > end) throw new Error(`timed out waiting for a frame in ${current}`)
        await sleep(100)
      }
    },
    close: () => ws.close(),
  }
}

async function history(topic, channel = CHANNEL) {
  const res = await fetch(`${HTTP}/api/v1/agents/${AGENT}/channel/${channel}/topic/${topic}/history?limit=500`, {
    headers: { authorization: `Bearer ${TOKEN}` },
  })
  return (await res.json().catch(() => ({}))).data ?? []
}
const kindOf = (entry) => Object.keys(entry.content)[0]
const requestKind = (entry) => (entry.content.Request ? Object.keys(entry.content.Request.content)[0] : null)
async function historyUntil(topic, predicate, timeoutMs = 60000, channel = CHANNEL) {
  const end = Date.now() + timeoutMs
  for (;;) {
    const entries = await history(topic, channel).catch(() => [])
    if (predicate(entries)) return entries
    if (Date.now() > end) return entries
    await sleep(300)
  }
}
const isReply = (f) => typeof f?.content?.message?.content === 'string'
const deliveries = (entries) =>
  entries.filter((e) => requestKind(e) === 'detached_delivery' && JSON.stringify(e.content).includes('[Subagent update]'))

/** The `result` of the conversation's last `execute_python` report. */
function lastResult(entries) {
  const results = entries.filter((e) => e.content.ToolResult)
  const raw = results.at(-1)?.content.ToolResult.content
  try {
    const report = typeof raw === 'string' ? JSON.parse(raw) : raw
    return report?.result ?? report
  } catch {
    return raw
  }
}

/** Send one script on a fresh topic's socket, wait for the reply, return its result. */
async function run(ws, topic, code) {
  const before = ws.frames.length
  ws.send({ chat: py(code) })
  await ws.until((f, i) => isReply(f) && ws.frames.indexOf(f) >= before)
  await sleep(300)
  return lastResult(await history(topic))
}

// --- setup ----------------------------------------------------------------------------------------
if (!TOKEN) {
  const login = await api('POST', '/auth/login', { username: USER, password: process.env.VIZIER_SETUP_PASSWORD ?? 'e2e-password' })
  TOKEN = login.data?.token
  if (!TOKEN) {
    const setup = await api('POST', '/auth/setup', { username: USER, password: process.env.VIZIER_SETUP_PASSWORD ?? 'e2e-password' })
    TOKEN = setup.data?.token
  }
}
check('signed in', !!TOKEN)
if ((await api('GET', `/agents/${AGENT}`)).status !== 200) {
  const made = await api('POST', '/agents', {
    agent_id: AGENT,
    name: 'Workers',
    provider: 'dummyplug',
    model: 'dummyplug',
    tools: { timeout: '5m' },
  })
  check('agent created', made.status < 300, made.body)
  // The local shell is an integration (spec 016), not an agent field.
  const shell = await api('POST', `/agents/${AGENT}/integrations`, {
    kind: 'shell',
    name: 'shell',
    settings: { path: '/tmp' },
    enabled: true,
  })
  check('a local shell is attached', shell.status < 300, shell.body)
  await sleep(2000)
}

// --- §2/§3 spawn returns at once, and a worker runs its task --------------------------------------
section('§3 spawn returns at once')
let topic = fresh('spawn')
let ws = connect(topic)
await ws.opened
let started = Date.now()
let r = await run(ws, topic, `spawn_subagents(tasks=[{'task': ${lit(slow(4))}}, {'task': ${lit(QUICK)}}])`)
check('the call returned two handles in order', r?.subagents?.length === 2 && r.subagents[0].position === 1 && r.subagents[1].position === 2, r)
check('both running', r.subagents.every((w) => w.state === 'running'), r)
check('returned without waiting for the 4 s worker', Date.now() - started < 4000, Date.now() - started)
const quickWorker = r.subagents[1]
let workerHistory = await historyUntil(quickWorker.topic, (es) => es.some((e) => e.content.Response), 30000, 'SUBAGENT')
check('§2 the worker got a subagent_task request', requestKind(workerHistory[0]) === 'subagent_task', workerHistory.map(kindOf))
check('§2 the worker ran its task and answered', workerHistory.some((e) => e.content.ToolCall) && workerHistory.some((e) => e.content.Response), workerHistory.map(kindOf))
// Both answers arrive as deliveries: nothing waited for them.
let entries = await historyUntil(topic, (es) => deliveries(es).length >= 1 && JSON.stringify(deliveries(es)).includes(r.subagents[0].id), 30000)
check('unwaited answers are delivered as [Subagent update]', deliveries(entries).length >= 1, entries.map(requestKind))
ws.close()

// --- §4 wait for any, stop one, wait for all ------------------------------------------------------
section('§4 wait any, stop one, wait all')
topic = fresh('stop')
ws = connect(topic)
await ws.opened
r = await run(
  ws,
  topic,
  [
    `r = spawn_subagents(tasks=[{'task': ${lit(QUICK)}}, {'task': ${lit(slow(30))}}])`,
    `ids = [w['id'] for w in r['subagents']]`,
    `first = wait_subagents(ids=ids, until='any')`,
    `stopped = stop_subagent(id=ids[1])`,
    `last = wait_subagents(ids=ids, until='all')`,
    `{'first': first, 'stopped': stopped, 'last': last}`,
  ].join('\n'),
)
check('wait any is met by the quick worker', r?.first?.met === true && r.first.subagents[0].state === 'answered', r?.first)
check('the slow one was still running', r.first.subagents[1].state === 'running', r.first)
check('stop_subagent stopped it, by the agent', r.stopped?.subagent?.state === 'stopped' && r.stopped.subagent.stop_reason === 'by_agent', r.stopped)
check('wait all is then met', r.last?.met === true, r.last)
await sleep(3000)
entries = await history(topic)
check('nothing is delivered: one was read by a wait, one was stopped', deliveries(entries).length === 0, entries.map(requestKind))
ws.close()

// --- §5 a worker asks, the main agent replies --------------------------------------------------------
section('§5 ask and reply')
topic = fresh('ask')
ws = connect(topic)
await ws.opened
r = await run(
  ws,
  topic,
  [
    `r = spawn_subagents(tasks=[{'task': ${lit(py("ask_main_agent(question='which tier?')"))}}])`,
    `sid = r['subagents'][0]['id']`,
    `asked = wait_subagents(ids=[sid])`,
    `replied = reply_subagent(id=sid, text='tools')`,
    `done = wait_subagents(ids=[sid])`,
    `again = reply_subagent(id=sid, text='more')`,
    `{'asked': asked, 'replied': replied, 'done': done, 'again': again}`,
  ].join('\n'),
)
check('the wait returns the question', r?.asked?.subagents?.[0]?.state === 'asking' && r.asked.subagents[0].question === 'which tier?', r?.asked)
check('the reply sets it running', r.replied?.subagent?.state === 'running', r.replied)
check('it answers after the reply', r.done?.subagents?.[0]?.state === 'answered', r.done)
workerHistory = await history(r.done.subagents[0].topic, 'SUBAGENT')
check('its answer is the reply handled in its own conversation', (r.done.subagents[0].result ?? '').includes('Available tools'), { result: r.done.subagents[0].result, history: workerHistory.map((e) => e.content) })
check('a second reply is refused: not asking', r.again?.status === 'not_asking', r.again)
const kinds = workerHistory.map((e) => requestKind(e) ?? kindOf(e))
check('one conversation: task, the ask, the reply, the answer', kinds.indexOf('subagent_task') === 0 && kinds.includes('subagent_reply') && kinds.at(-1) === 'Response', kinds)
check('the socket carried subagent frames', ws.frames.some((f) => f.subagent?.id === r.done.subagents[0].id && f.subagent.state === 'asking'))

// §13 (API part): a person reads the topic's workers, one worker, and its own conversation.
const sid = r.done.subagents[0].id
const base = `/agents/${AGENT}/channel/${CHANNEL}/topic/${topic}/subagents`
const listed = await api('GET', base)
check('the topic lists its worker', listed.status === 200 && listed.data?.some((w) => w.id === sid && w.state === 'answered'), listed.body)
check('the main agent has its outcome', listed.data.find((w) => w.id === sid)?.handed_over === true, listed.data)
const one = await api('GET', `${base}/${sid}`)
check('one worker reads whole', one.status === 200 && one.data?.result?.includes('Available tools') && one.data.group, one.body)
const steps = await api('GET', `${base}/${sid}/history`)
check('its conversation reads through the hiring topic', steps.status === 200 && steps.data?.length === workerHistory.length, { got: steps.data?.length, want: workerHistory.length })
const elsewhere = await api('GET', `/agents/${AGENT}/channel/${CHANNEL}/topic/${fresh('other')}/subagents/${sid}`)
check('another topic cannot read it', elsewhere.status === 404, elsewhere.body)
const unknown = await api('GET', `${base}/s_nothere0`)
check('an unknown id is not found', unknown.status === 404, unknown.body)
ws.close()

// --- §6 workers survive the parent's /abort ------------------------------------------------------------
section('§6 /abort stops only the main agent')
topic = fresh('abort')
ws = connect(topic)
await ws.opened
ws.send({ chat: py(`r = spawn_subagents(tasks=[{'task': ${lit(slow(6))}}])\nshell_exec(commands='sleep 30')\nr`) })
await ws.until((f) => f.history_entry && kindOf(f.history_entry) === 'ToolCall')
await sleep(1500)
ws.send({ command: 'abort' })
await ws.until((f) => f.content === 'abort')
entries = await historyUntil(topic, (es) => deliveries(es).length >= 1, 40000)
check('the worker kept running and its answer was delivered', deliveries(entries).length === 1 && JSON.stringify(deliveries(entries)).includes('state: answered'), entries.map(requestKind))
await sleep(3000)
check('delivered once', deliveries(await history(topic)).length === 1)
ws.close()

// --- §7 mid-turn delivery ------------------------------------------------------------------------------
section('§7 mid-turn delivery')
topic = fresh('midturn')
ws = connect(topic)
await ws.opened
ws.send({ chat: py(`spawn_subagents(tasks=[{'task': ${lit(QUICK)}}])\nshell_exec(commands='sleep 6')\n'waited'`) })
await ws.until(isReply, 60000)
await sleep(4000)
entries = await history(topic)
const firstResponse = entries.findIndex((e) => e.content.Response)
const delivery = entries.findIndex((e) => deliveries([e]).length === 1)
check('the answer was delivered inside the running turn', delivery > 0 && delivery < firstResponse, entries.map((e) => requestKind(e) ?? kindOf(e)))
check('no extra turn started for it', deliveries(entries).length === 1 && entries.filter((e) => e.content.Request).length === 2, entries.map((e) => requestKind(e) ?? kindOf(e)))
ws.close()

// --- §8 restart resumes a worker ---------------------------------------------------------------------
if (process.env.VIZIER_STOP && process.env.VIZIER_START) {
  section('§8 a restart resumes a worker')
  topic = fresh('restart')
  ws = connect(topic)
  await ws.opened
  r = await run(ws, topic, `spawn_subagents(tasks=[{'task': ${lit(slow(12))}}])`)
  const restarted = r.subagents[0]
  await historyUntil(restarted.topic, (es) => es.some((e) => e.content.ToolCall), 20000, 'SUBAGENT')
  ws.close()
  execSync(process.env.VIZIER_STOP, { stdio: 'inherit' })
  execSync(process.env.VIZIER_START, { stdio: 'inherit' })
  for (let i = 0; i < 60; i++) {
    if ((await api('GET', `/agents/${AGENT}`).catch(() => ({ status: 0 }))).status === 200) break
    await sleep(1000)
  }
  entries = await historyUntil(topic, (es) => deliveries(es).length >= 1, 90000)
  check('the worker resumed and was delivered once', deliveries(entries).length === 1, entries.map(requestKind))
  check('its outcome reads answered', JSON.stringify(deliveries(entries)).includes('state: answered'), deliveries(entries))
} else {
  console.log('§8 skipped (VIZIER_STOP / VIZIER_START not set)')
}

// --- §9 only the main agent hires ------------------------------------------------------------------------
section('§9 only the main agent hires')
topic = fresh('depth')
ws = connect(topic)
await ws.opened
r = await run(
  ws,
  topic,
  [
    `mine = list_tools(category='subagents')`,
    `r = spawn_subagents(tasks=[{'task': ${lit(py("list_tools(category='subagents')"))}}])`,
    `w = wait_subagents(ids=[r['subagents'][0]['id']])`,
    `{'mine': mine, 'worker': w['subagents'][0]['result']}`,
  ].join('\n'),
)
const mine = JSON.stringify(r?.mine)
check('the main agent sees the six management tools', ['spawn_subagents', 'wait_subagents', 'reply_subagent', 'stop_subagent', 'list_subagents', 'get_subagent'].every((n) => mine.includes(n)), r?.mine)
check('and not ask_main_agent', !mine.includes('ask_main_agent'), r?.mine)
const theirs = String(r?.worker ?? '')
check('a worker sees only ask_main_agent', theirs.includes('ask_main_agent') && !theirs.includes('spawn_subagents'), theirs)
ws.close()

// --- §10 retired names ---------------------------------------------------------------------------------
section('§10 retired names')
topic = fresh('retired')
ws = connect(topic)
await ws.opened
for (const name of ['parallel_subagents', 'paralel_subtasks']) {
  ws.send({ chat: py(`${name}(tasks=[{'prompt': 'x'}])`) })
  await sleep(2500)
}
entries = await historyUntil(topic, (es) => es.filter((e) => e.content.ToolResult).length >= 2, 30000)
const texts = entries.filter((e) => e.content.ToolResult).map((e) => JSON.stringify(e.content.ToolResult))
check('both name the replacement tools', texts.length >= 2 && texts.every((t) => t.includes('spawn_subagents') && t.includes('wait_subagents')), texts)
ws.close()

// --- §11 deleting the conversation stops its workers ---------------------------------------------------
section('§11 deleting the conversation')
topic = fresh('delete')
ws = connect(topic)
await ws.opened
r = await run(ws, topic, `spawn_subagents(tasks=[{'task': ${lit(slow(20))}}])`)
const doomed = r.subagents[0]
await historyUntil(doomed.topic, (es) => es.some((e) => e.content.ToolCall), 20000, 'SUBAGENT')
ws.close()
await sleep(500)
const deleted = await api('DELETE', `/agents/${AGENT}/channel/${CHANNEL}/topic/${topic}`)
check('the conversation is deleted', deleted.status < 300, deleted.body)
await sleep(3000)
check('its worker conversation is gone', (await history(doomed.topic, 'SUBAGENT')).length === 0)
await sleep(20000)
check('nothing came back to life to deliver', (await history(topic)).length === 0)

console.log('all subagent checks passed')
