#!/usr/bin/env node
// End-to-end check of the inbox stream (specs/051-realtime-notifications/quickstart.md steps 1-3)
// against a running vizier with an agent on the offline `dummyplug` provider.
//
//   VIZIER_URL=ws://localhost:9999 VIZIER_TOKEN=<jwt> AGENT=plug node scripts/inbox-e2e.mjs
//   QUIET_SECS=360 (default) is how long the quiet-socket check waits; 0 skips it.
//
// Dependency-free: Node >= 22 has a global WebSocket and fetch. Exits non-zero with the first
// failing check. Steps that need a task run, a model download or Discord/Telegram are not
// scripted here; quickstart.md lists them.

const BASE = (process.env.VIZIER_URL ?? 'ws://localhost:9999').replace(/\/$/, '')
const TOKEN = process.env.VIZIER_TOKEN
const AGENT = process.env.AGENT ?? 'plug'
const USER = process.env.VIZIER_USER ?? 'e2e'
const QUIET_SECS = Number(process.env.QUIET_SECS ?? 360)
const HTTP = BASE.replace(/^ws/, 'http')

if (!TOKEN) {
  console.error('VIZIER_TOKEN is required')
  process.exit(2)
}

let current = 'setup'
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${current} > ${name}${detail === undefined ? '' : `\n  ${JSON.stringify(detail).slice(0, 2000)}`}`)
    process.exit(1)
  }
  console.log(`  ok ${name}`)
}
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

/** A socket that records every JSON frame and lets a check wait for the next matching one. */
function open(url) {
  const ws = new WebSocket(url)
  const frames = []
  const waiters = []
  let closed = false
  ws.addEventListener('message', (event) => {
    let frame
    try {
      frame = JSON.parse(event.data)
    } catch {
      return
    }
    frames.push(frame)
    for (const w of [...waiters]) {
      if (w.predicate(frame)) {
        waiters.splice(waiters.indexOf(w), 1)
        w.resolve(frame)
      }
    }
  })
  ws.addEventListener('close', () => (closed = true))
  const opened = new Promise((resolve, reject) => {
    ws.addEventListener('open', resolve, { once: true })
    ws.addEventListener('error', reject, { once: true })
  })
  return {
    ws,
    frames,
    opened,
    get closed() {
      return closed
    },
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
            reject(new Error('timed out'))
          }
        }, timeoutMs)
      })
    },
  }
}

const stream = () => open(`${BASE}/api/v1/inbox/stream?token=${TOKEN}`)
const chat = (topic) => open(`${BASE}/api/v1/agents/${AGENT}/channel/e2e/topic/${topic}/chat?token=${TOKEN}`)
const request = (text) => ({ timestamp: new Date().toISOString(), user: USER, content: { chat: text }, metadata: {} })

async function api(method, path, body) {
  const res = await fetch(`${HTTP}/api/v1${path}`, {
    method,
    headers: { Authorization: `Bearer ${TOKEN}`, 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  return { status: res.status, json: await res.json().catch(() => null) }
}

// ------------------------------------------------------------------------------------ 1
current = '1. connecting'
const a = stream()
await a.opened
const hello = await a.next((f) => f.type === 'hello')
check('hello is the first frame', a.frames[0].type === 'hello')
check('hello carries counts, latest_id and setups', hello.counts && typeof hello.latest_id === 'number' && Array.isArray(hello.setups), hello)

if (QUIET_SECS > 0) {
  console.log(`  ... waiting ${QUIET_SECS}s to prove a quiet socket stays open past the chat idle timeout`)
  await sleep(QUIET_SECS * 1000)
  check('the quiet socket is still open', !a.closed)
}

// ------------------------------------------------------------------------------------ 2
current = '2. a reply notifies live'
const b = stream()
await b.opened
await b.next((f) => f.type === 'hello')
const before = a.frames.length
const topic = `inbox-e2e-${Date.now()}`
const c = chat(topic)
await c.opened
c.ws.send(JSON.stringify(request('tools')))
const note = await a.next((f) => f.type === 'notification' && f.item.category === 'chat_reply', 30000, before)
check('a chat_reply notification arrived', !!note)
check('it is unread and carries counts', note.item.read === false && note.counts.unread >= 1, note)
check('the excerpt is short plain text', note.item.excerpt === undefined || note.item.excerpt.length <= 141, note.item.excerpt)
check('the source session is never sent', !('source_session' in note.item))
const page = await api('GET', '/notifications?filter=unread')
check('the same row is in the unread list', page.json?.data?.items?.some((i) => i.id === note.item.id))

// ------------------------------------------------------------------------------------ 3
current = '3. read state crosses clients'
const mark = b.frames.length
const put = await api('PUT', `/notifications/${note.item.id}/read`, { read: true })
check('marking read succeeds', put.status === 200, put)
const read = await b.next((f) => f.type === 'read' && f.id === note.item.id, 5000, mark)
check('the person\'s other stream is told', read.read === true && typeof read.counts.unread === 'number', read)

console.log('inbox stream: all scripted checks passed')
a.ws.close()
b.ws.close()
c.ws.close()
process.exit(0)
