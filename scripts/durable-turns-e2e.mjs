#!/usr/bin/env node
// End-to-end check of durable turns (specs/023-persist-turn-activity/quickstart.md) against a
// running vizier with an agent on the offline `dummyplug` provider and a local shell.
//
//   VIZIER_URL=ws://localhost:9999 VIZIER_TOKEN=<jwt> AGENT=plug node scripts/durable-turns-e2e.mjs
//
// The restart checks run only when VIZIER_STOP and VIZIER_START are set: shell commands that
// kill vizier (e.g. `kill -9 $VIZIER_PID`) and start it again (detached). Dependency-free:
// Node ≥ 22 has a global WebSocket and fetch. Exits non-zero with the first failed check.

import { execSync } from 'node:child_process'

const BASE = (process.env.VIZIER_URL ?? 'ws://localhost:9999').replace(/\/$/, '')
const HTTP = BASE.replace(/^ws/, 'http')
const TOKEN = process.env.VIZIER_TOKEN
const AGENT = process.env.AGENT ?? 'plug'
const USER = process.env.VIZIER_USER ?? 'e2e'
const CHANNEL = 'e2e'

if (!TOKEN) {
  console.error('VIZIER_TOKEN is required')
  process.exit(2)
}

let current = 'setup'
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${current} › ${name}${detail === undefined ? '' : `\n  ${JSON.stringify(detail).slice(0, 2000)}`}`)
    process.exit(1)
  }
  console.log(`  ok ${name}`)
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
    send: (content) =>
      ws.send(JSON.stringify({ timestamp: new Date().toISOString(), user: USER, content, metadata: {} })),
    async until(predicate, timeoutMs = 20000) {
      const end = Date.now() + timeoutMs
      for (;;) {
        const found = frames.find(predicate)
        if (found) return found
        if (Date.now() > end) throw new Error('timed out waiting for a frame')
        await sleep(100)
      }
    },
    close: () => ws.close(),
  }
}

async function history(topic) {
  const res = await fetch(`${HTTP}/api/v1/agents/${AGENT}/channel/${CHANNEL}/topic/${topic}/history?limit=500`, {
    headers: { authorization: `Bearer ${TOKEN}` },
  })
  return (await res.json()).data ?? []
}
const kindOf = (entry) => Object.keys(entry.content)[0]
async function historyUntil(topic, predicate, timeoutMs = 30000) {
  const end = Date.now() + timeoutMs
  for (;;) {
    try {
      const entries = await history(topic)
      if (predicate(entries)) return entries
    } catch {}
    if (Date.now() > end) return history(topic)
    await sleep(300)
  }
}

const shell = (commands) => ({
  chat: JSON.stringify({ tool: 'execute_python', arguments: { intent: 'e2e', code: `shell_exec(commands=${JSON.stringify(commands)})` } }),
})
const isHistoryEntry = (kind) => (f) => f.history_entry && kindOf(f.history_entry) === kind
const isReply = (f) => typeof f?.content?.message?.content === 'string'
const fresh = (name) => `${name}-${Date.now()}`

// §1 — every step is pushed once, and equals what history holds.
current = '§1 steps are pushed as they happen'
{
  const topic = fresh('steps')
  const ws = connect(topic)
  await ws.opened
  ws.send(shell('echo hi'))
  await ws.until(isReply)
  await sleep(300)
  const pushed = ws.frames.filter((f) => f.history_entry).map((f) => f.history_entry)
  const stored = await history(topic)
  check('a ToolCall and a ToolResult were pushed', pushed.some((e) => kindOf(e) === 'ToolCall') && pushed.some((e) => kindOf(e) === 'ToolResult'), pushed.map(kindOf))
  check('each pushed step once', new Set(pushed.map((e) => e.uid)).size === pushed.length)
  check('every pushed step is in history', pushed.every((e) => stored.some((s) => s.uid === e.uid)))
  check('history order is Request, steps, Response', kindOf(stored[0]) === 'Request' && kindOf(stored.at(-1)) === 'Response', stored.map(kindOf))
  ws.close()
}

// §2 — a second socket on a running turn sees the steps (here: through history merge).
current = '§2 a late reader gets every step'
{
  const topic = fresh('late')
  const ws = connect(topic)
  await ws.opened
  ws.send(shell('sleep 3; echo late'))
  await ws.until(isHistoryEntry('ToolCall'))
  const late = connect(topic)
  await late.opened
  const read = await history(topic)
  check('the open call is already in history', read.some((e) => kindOf(e) === 'ToolCall'), read.map(kindOf))
  await late.until(isHistoryEntry('ToolResult'))
  check('the late socket gets the result as a frame', true)
  await ws.until(isReply)
  ws.close()
  late.close()
}

// §6b — an abort mid-call leaves no call without a result, and the next message works.
current = '§6b abort seals the open call'
{
  const topic = fresh('abort')
  const ws = connect(topic)
  await ws.opened
  ws.send(shell('sleep 30'))
  await ws.until(isHistoryEntry('ToolCall'))
  ws.send({ command: 'abort' })
  await ws.until((f) => f.content === 'abort')
  const entries = await historyUntil(topic, (es) => es.some((e) => e.content.ToolResult?.content.startsWith('Stopped:')))
  const calls = entries.filter((e) => e.content.ToolCall).map((e) => e.content.ToolCall.call_id)
  const results = entries.filter((e) => e.content.ToolResult).map((e) => e.content.ToolResult.call_id)
  check('every call has a result', calls.every((id) => results.includes(id)), entries.map(kindOf))
  const mark = ws.frames.length
  ws.send({ chat: 'tools' })
  await ws.until((f, i) => isReply(f), 20000)
  check('the next message is answered', ws.frames.length > mark)
  ws.close()
}

// §3/§4 — a restart mid-call resumes the turn.
if (process.env.VIZIER_STOP && process.env.VIZIER_START) {
  current = '§3 a kill mid-call resumes the turn'
  const topic = fresh('restart')
  const ws = connect(topic)
  await ws.opened
  ws.send(shell('sleep 30'))
  await ws.until(isHistoryEntry('ToolCall'))
  execSync(process.env.VIZIER_STOP, { stdio: 'inherit', shell: '/bin/sh' })
  ws.close()
  execSync(process.env.VIZIER_START, { stdio: 'inherit', shell: '/bin/sh' })
  for (let i = 0; i < 60; i += 1) {
    try {
      if ((await fetch(`${HTTP}/api/v1/ping`)).ok) break
    } catch {}
    await sleep(500)
  }
  const entries = await historyUntil(topic, (es) => es.at(-1) && kindOf(es.at(-1)) === 'Response', 60000)
  check('the cut-off call is closed with the restart notice', entries.some((e) => e.content.ToolResult?.content.startsWith('Interrupted:')), entries.map(kindOf))
  check('the turn finished with a reply', kindOf(entries.at(-1)) === 'Response', entries.map(kindOf))
  check('the reply is marked resumed', entries.at(-1).content.Response.resumed === true)
  check('the request is in history once', entries.filter((e) => e.content.Request).length === 1)

  current = '§5 a queued message survives the restart'
  const queued = fresh('queued')
  const q = connect(queued)
  await q.opened
  q.send(shell('sleep 30'))
  await q.until(isHistoryEntry('ToolCall'))
  q.send({ chat: 'tools' })
  await sleep(500)
  execSync(process.env.VIZIER_STOP, { stdio: 'inherit', shell: '/bin/sh' })
  q.close()
  execSync(process.env.VIZIER_START, { stdio: 'inherit', shell: '/bin/sh' })
  for (let i = 0; i < 60; i += 1) {
    try {
      if ((await fetch(`${HTTP}/api/v1/ping`)).ok) break
    } catch {}
    await sleep(500)
  }
  const after = await historyUntil(queued, (es) => es.filter((e) => e.content.Response).length >= 2, 60000)
  check('both requests were answered', after.filter((e) => e.content.Response).length === 2, after.map(kindOf))
  check('in the order they were sent', after.filter((e) => e.content.Request).map((e) => JSON.stringify(e.content.Request.content))[1].includes('tools'))
} else {
  console.log('  skip restart checks (set VIZIER_STOP and VIZIER_START to run them)')
}

console.log('all durable-turn checks passed')
process.exit(0)
