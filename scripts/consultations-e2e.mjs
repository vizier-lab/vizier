#!/usr/bin/env node
// End-to-end check of consultations (specs/064-consult-agents/quickstart.md §1 to §9) against a
// running vizier with agents on the offline `dummyplug` provider.
//
//   dummyplug:  cd dummyplug && node src/server.mjs        (listens on :9998)
//   vizier:     just run                                   (or any server on VIZIER_URL)
//   this:       VIZIER_URL=ws://localhost:9999 VIZIER_SETUP_USER=e2e node scripts/consultations-e2e.mjs
//
// The consulted agent is on dummyplug too, so the request text `alpha` sends **is** a dummyplug
// message: `{"say": "pong"}` makes `beta` answer `pong`, and an `execute_python` request makes it
// run that tool (to ask back). The person-and-access cases that need a second person are pinned in
// crates/vizier-core/src/consultations/access.rs; restarts and deletion in
// crates/vizier-core/tests/dummyplug_e2e.rs. Dependency-free: Node ≥ 22. Exits non-zero at the
// first failed check.

const BASE = (process.env.VIZIER_URL ?? 'ws://localhost:9999').replace(/\/$/, '')
const HTTP = BASE.replace(/^ws/, 'http')
let TOKEN = process.env.VIZIER_TOKEN
const USER = process.env.VIZIER_SETUP_USER ?? 'e2e'
const CHANNEL = 'e2e'
const ASKER = 'alpha'
const CONSULTEE = 'beta'

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
/** What the consultee answers: `say`, after `delayMs`. */
const answer = (say, delayMs = 0) => JSON.stringify({ say, delay_ms: delayMs })
/** A request that makes the consultee ask the agent consulting it. */
const askBack = (question) => py(`ask_consulting_agent(question=${lit(question)})`)

function connect(topic) {
  const ws = new WebSocket(`${BASE}/api/v1/agents/${ASKER}/channel/${CHANNEL}/topic/${topic}/chat?token=${TOKEN}`)
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
    send: (content) => ws.send(JSON.stringify({ timestamp: new Date().toISOString(), user: USER, content, metadata: {} })),
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

async function history(agent, channel, topic) {
  const res = await fetch(`${HTTP}/api/v1/agents/${agent}/channel/${encodeURIComponent(channel)}/topic/${topic}/history?limit=500`, {
    headers: { authorization: `Bearer ${TOKEN}` },
  })
  return (await res.json().catch(() => ({}))).data ?? []
}
const requestKind = (entry) => (entry.content.Request ? Object.keys(entry.content.Request.content)[0] : null)
async function historyUntil(agent, channel, topic, predicate, timeoutMs = 60000) {
  const end = Date.now() + timeoutMs
  for (;;) {
    const entries = await history(agent, channel, topic).catch(() => [])
    if (predicate(entries)) return entries
    if (Date.now() > end) return entries
    await sleep(300)
  }
}
const isReply = (f) => typeof f?.content?.message?.content === 'string'
const deliveries = (entries) =>
  entries.filter((e) => requestKind(e) === 'detached_delivery' && JSON.stringify(e.content).includes('[Consultation update]'))

/** The `result` of the conversation's last `execute_python` report. */
function lastResult(entries) {
  const raw = entries.filter((e) => e.content.ToolResult).at(-1)?.content.ToolResult.content
  try {
    const report = typeof raw === 'string' ? JSON.parse(raw) : raw
    return report?.result ?? report
  } catch {
    return raw
  }
}

/** Send one script on the topic's socket, wait for the reply, return its result. */
async function run(ws, topic, code) {
  const before = ws.frames.length
  ws.send({ chat: py(code) })
  await ws.until((f) => isReply(f) && ws.frames.indexOf(f) >= before)
  await sleep(300)
  return lastResult(await history(ASKER, CHANNEL, topic))
}
const consult = (request, extra = '') =>
  `consult_agent(consultations=[{'agent': ${lit(CONSULTEE)}, 'request': ${lit(request)}}]${extra})`

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
for (const id of [ASKER, CONSULTEE]) {
  if ((await api('GET', `/agents/${id}`)).status !== 200) {
    const made = await api('POST', '/agents', { agent_id: id, name: id, provider: 'dummyplug', model: 'dummyplug', tools: { timeout: '5m' } })
    check(`agent ${id} created`, made.status < 300, made.body)
  }
}
await sleep(2000) // the agents start in the background

// --- §1 consult and wait --------------------------------------------------------------------------
section('§1 consult and wait')
let topic = fresh('consult')
let ws = connect(topic)
await ws.opened
let r = await run(ws, topic, `r = ${consult(answer('pong'))}\nwait_consultations(ids=[x['id'] for x in r['consultations']])`)
check('the wait returned the answer', r?.met === true && r.consultations?.[0]?.state === 'answered' && r.consultations[0].result === 'pong', r)
check('it names the consulted agent', r.consultations[0].consultee === CONSULTEE, r)
const first = r.consultations[0]
const theirs = await history(CONSULTEE, `consultation__${ASKER}`, first.topic)
check("the consultee's own conversation holds it", theirs.some((e) => requestKind(e) === 'subagent_task'), theirs)
const listed = await api('GET', `/agents/${CONSULTEE}/conversations?channel=consultation`)
check('the consultee lists "Asked by" for its editor', listed.data?.items?.some((i) => i.kind === 'consultation' && i.title.startsWith('Asked by')), listed.body)
const strip = await api('GET', `/agents/${ASKER}/channel/${CHANNEL}/topic/${topic}/subagents`)
check('the topic lists the consultation with its consultee', strip.data?.some?.((w) => w.id === first.id && w.consultee === CONSULTEE), strip.body)
ws.close()

// --- §2 delivery without a wait -------------------------------------------------------------------
section('§2 delivered once when nobody waits')
topic = fresh('later')
ws = connect(topic)
await ws.opened
r = await run(ws, topic, consult(answer('late', 2000)))
const late = r?.consultations?.[0]
check('returned a handle at once', late?.state === 'running', r)
let entries = await historyUntil(ASKER, CHANNEL, topic, (es) => deliveries(es).length > 0)
check('one delivery arrived', deliveries(entries).length === 1, entries.map(requestKind))
check('it carries the answer', JSON.stringify(deliveries(entries)[0]).includes('late'))
await sleep(2000)
check('and only once', deliveries(await history(ASKER, CHANNEL, topic)).length === 1)
ws.close()

// --- §3 stop one ----------------------------------------------------------------------------------
section('§3 stop one, the other finishes')
topic = fresh('stop')
ws = connect(topic)
await ws.opened
r = await run(
  ws,
  topic,
  `r = consult_agent(consultations=[{'agent': ${lit(CONSULTEE)}, 'request': ${lit(answer('a', 3000))}}, {'agent': ${lit(CONSULTEE)}, 'request': ${lit(answer('b', 3000))}}])
ids = [x['id'] for x in r['consultations']]
stop_consultation(id=ids[1])
wait_consultations(ids=ids)`,
)
check('the first answered', r?.consultations?.[0]?.state === 'answered', r)
check('the second was stopped', r.consultations?.[1]?.state === 'stopped' && r.consultations[1].stop_reason === 'by_agent', r)
ws.close()

// --- §4 asking back and following up --------------------------------------------------------------
section('§4 ask back, reply, follow up')
topic = fresh('talk')
ws = connect(topic)
await ws.opened
r = await run(ws, topic, `r = ${consult(askBack('eu or us?'))}\nwait_consultations(ids=[x['id'] for x in r['consultations']])`)
const talk = r?.consultations?.[0]
check('it is asking', talk?.state === 'asking' && talk.question === 'eu or us?', r)
r = await run(ws, topic, `reply_consultation(id=${lit(talk.id)}, text=${lit(answer('eu it is'))})\nwait_consultations(ids=[${lit(talk.id)}])`)
check('the reply was answered', r?.consultations?.[0]?.result === 'eu it is', r)
r = await run(ws, topic, `reply_consultation(id=${lit(talk.id)}, text=${lit(answer('and again'))})\nwait_consultations(ids=[${lit(talk.id)}])`)
check('a follow-up continues it', r?.consultations?.[0]?.result === 'and again' && r.consultations[0].follow_ups === 1, r)
ws.close()

// --- §5 refusals ----------------------------------------------------------------------------------
section('§5 refusals')
topic = fresh('refuse')
ws = connect(topic)
await ws.opened
r = await run(ws, topic, `consult_agent(consultations=[{'agent': 'no-such-agent', 'request': 'hi'}])`)
check('an unknown agent', r?.status === 'unknown_agent', r)
r = await run(ws, topic, `consult_agent(consultations=[{'agent': ${lit(ASKER)}, 'request': 'hi'}])`)
check('itself', r?.status === 'self', r)
r = await run(ws, topic, 'list_consultable_agents()')
check('the consultable list has the consultee and not the asker', r?.agents?.some((a) => a.id === CONSULTEE) && !r.agents.some((a) => a.id === ASKER), r)
r = await run(ws, topic, "get_subagent(id='c_00000000')")
check("the workers' tools do not take a consultation's handle", r?.status === 'unknown_subagent', r)
ws.close()

// --- §9 the Workspace names are retired -----------------------------------------------------------
section('§9 Workspaces are gone')
check('the Workspace routes are gone', (await api('GET', '/workspaces')).status === 404)
topic = fresh('retired')
ws = connect(topic)
await ws.opened
r = await run(ws, topic, "try:\n    mention_agent(agents=['beta'], message='hi')\nexcept Exception as e:\n    str(e)")
check('mention_agent points at consult_agent', typeof r === 'string' && r.includes('consult_agent') && r.includes('wait_consultations'), r)
ws.close()

console.log('all consultation checks passed')
