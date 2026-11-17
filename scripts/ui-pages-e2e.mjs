#!/usr/bin/env node
// End-to-end check of agent pages through the real topic socket (specs/063-html-generative-ui
// quickstart §0 and §3–§5) against a running vizier with an agent on the offline `dummyplug`
// provider. The script's own socket is a fake tab: it answers `ui_verify` from a script instead
// of mounting the page, so verification is checked without a browser.
//
//   VIZIER_URL=ws://localhost:9999 VIZIER_TOKEN=<jwt> AGENT=plug node scripts/ui-pages-e2e.mjs
//   node scripts/ui-pages-e2e.mjs --no-tab     # only §0 and §1: nothing answers ui_verify
//
// Dependency-free: Node ≥ 22 has a global WebSocket and fetch. Exits non-zero with the name of
// the first check that fails.

const BASE = (process.env.VIZIER_URL ?? 'ws://localhost:9999').replace(/\/$/, '')
const TOKEN = process.env.VIZIER_TOKEN
const AGENT = process.env.AGENT ?? 'plug'
const USER = process.env.VIZIER_USER ?? 'e2e'
const CHANNEL = 'vizier-webui'
const HTTP = BASE.replace(/^ws/, 'http')
const NO_TAB = process.argv.includes('--no-tab')

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
const kind = (frame, key) => frame && Object.prototype.hasOwnProperty.call(frame, key)
const reply = (frame) => frame?.content?.message?.content
const isReply = (frame) => typeof reply(frame) === 'string'

/**
 * A socket on `topic`. `answer(request)` decides what the fake tab reports for each `ui_verify`:
 * `{problems}` to report, or `null` to stay silent. Without it the socket never reports.
 */
function connect(topic, answer) {
  const url = `${BASE}/api/v1/agents/${AGENT}/channel/${CHANNEL}/topic/${topic}/chat?token=${TOKEN}`
  const ws = new WebSocket(url)
  const frames = []
  const waiters = []
  const send = (frame) => ws.send(JSON.stringify(frame))
  ws.addEventListener('message', (event) => {
    let frame
    try {
      frame = JSON.parse(event.data)
    } catch {
      return
    }
    frames.push(frame)
    if (kind(frame, 'ui_verify') && answer) {
      const said = answer(frame.ui_verify)
      if (said) {
        setTimeout(
          () => send({ ui_verify_report: { request_id: frame.ui_verify.request_id, problems: said.problems ?? [], observed_ms: frame.ui_verify.observe_ms } }),
          said.delayMs ?? 50
        )
      }
    }
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
    send,
    next(predicate, timeoutMs = 15000, from = 0) {
      const seen = frames.slice(from).find(predicate)
      if (seen) return Promise.resolve(seen)
      return new Promise((resolve, reject) => {
        const waiter = { predicate, resolve }
        waiters.push(waiter)
        setTimeout(() => {
          const at = waiters.indexOf(waiter)
          if (at >= 0) {
            waiters.splice(at, 1)
            reject(new Error(`${current}: no matching frame within ${timeoutMs} ms`))
          }
        }, timeoutMs)
      })
    },
    close() {
      ws.close()
    },
  }
}

const request = (content, extra = {}) => ({ timestamp: new Date().toISOString(), user: USER, content, metadata: {}, ...extra })
const runTool = (tool, args) => ({ chat: JSON.stringify({ tool, arguments: args }) })
const python = (code) => runTool('execute_python', { code, intent: 'e2e' })
const py = (value) => JSON.stringify(value)
const H = '<h1>Hi</h1><button id=b>Go</button>'

async function api(path) {
  const res = await fetch(`${HTTP}/api/v1/agents/${AGENT}/channel/${CHANNEL}/topic/${path}`, { headers: { authorization: `Bearer ${TOKEN}` } })
  const body = await res.json()
  return body.data ?? body
}
const history = (topic) => api(`${topic}/history?limit=500`)
const pagesIn = (entries) => entries.filter((e) => e.content?.UiPage).map((e) => e.content.UiPage)

/** Send `code` and wait for the turn's end: the reply frame. */
async function turn(socket, code, timeoutMs = 30000) {
  const from = socket.frames.length
  socket.send(request(python(code)))
  const end = await socket.next(isReply, timeoutMs, from)
  return { from, text: reply(end) }
}

/** The `ui_page_verified` frame for the newest version drawn since `from`. */
const verifiedSince = (socket, from, timeoutMs = 15000) => socket.next((f) => kind(f, 'ui_page_verified'), timeoutMs, from)

const fresh = () => crypto.randomUUID()

// §0 ---------------------------------------------------------------------------------------
current = '§0 availability'
{
  const s = connect(fresh())
  await s.opened
  const from = s.frames.length
  s.send(request(runTool('list_tool_functions', { category: 'ui' })))
  const listed = reply(await s.next(isReply, 15000, from))
  check('ui_render is offered in a WebUI chat', listed.includes('ui_render') && !listed.includes('did_you_mean'), listed.slice(0, 600))
  const retired = await turn(s, 'ui_components()')
  check('the retired client tool names ui_render', retired.text.includes('ui_render'), retired.text)
  s.close()
}

// §1 ---------------------------------------------------------------------------------------
current = '§1 no tab'
{
  // The message is sent and its socket closed at once, so the turn runs with no tab on the
  // conversation (an open socket that never reports would read as a timeout instead). The
  // result is read back from storage.
  const topic = fresh()
  const sender = connect(topic)
  await sender.opened
  sender.send(request(python(`ui_render(title="Hello", html=${py(H)})`)))
  await sleep(100)
  sender.close()
  let pages = []
  for (let i = 0; i < 60 && pages.length === 0; i++) {
    await sleep(250)
    pages = pagesIn(await history(topic))
  }
  check('a page was drawn with no tab', pages.length === 1, pages)
  const page = await api(`${topic}/pages/${pages[0].page_id}`)
  const state = page.versions?.[0]?.verification?.state ?? pages[0].verification?.state
  check('it is unverified', state === 'unverified', page)
}

if (NO_TAB) {
  console.log('done (--no-tab)')
  process.exit(0)
}

// §3 ---------------------------------------------------------------------------------------
current = '§3 clean report'
{
  const s = connect(fresh(), () => ({ problems: [] }))
  await s.opened
  const from = s.frames.length
  s.send(request(python(`ui_render(title="Ok", html=${py(H)})`)))
  const verified = await verifiedSince(s, from)
  check('ui_page_verified is clean', verified.ui_page_verified.verification?.state === 'clean', verified)
  check('the check went to this socket', s.frames.slice(from).some((f) => kind(f, 'ui_verify')))
  s.close()
}

current = '§3 problems, then a fix in the same turn'
{
  const topic = fresh()
  let reports = 0
  const s = connect(topic, () => (reports++ === 0 ? { problems: [{ kind: 'error', message: 'boom', line: 3 }] } : { problems: [] }))
  await s.opened
  const { from, text } = await turn(s, `ui_render(title="Bad", html=${py(H)})`)
  check('the refusal reaches the model as problems', text.includes('problems') && text.includes('boom'), text)
  const bad = await verifiedSince(s, from)
  check('the version is saved with its problems', bad.ui_page_verified.verification?.state === 'problems', bad)
  const pageId = bad.ui_page_verified.page_id
  const fix = await turn(s, `ui_render(page=${py(pageId)}, base_version=1, edits=[{"find": "Hi", "replace": "Hello"}])`)
  const good = await s.next((f) => kind(f, 'ui_page_verified') && f.ui_page_verified.version === 2, 15000, fix.from)
  check('the fix is clean', good.ui_page_verified.verification?.state === 'clean', good)
  const pages = pagesIn(await history(topic))
  check('history has both versions', pages.map((p) => p.version).join(',') === '1,2', pages)
  s.close()
}

current = '§3 silence'
{
  const s = connect(fresh(), () => null)
  await s.opened
  const from = s.frames.length
  s.send(request(python(`ui_render(title="Slow", html=${py(H)})`)))
  const verified = await verifiedSince(s, from, 20000)
  const v = verified.ui_page_verified.verification
  check('a silent tab times out', v?.state === 'unverified' && v.reason === 'timeout', verified)
  s.close()
}

current = '§3 two tabs, first reply wins'
{
  const topic = fresh()
  const a = connect(topic, () => ({ problems: [], delayMs: 50 }))
  const b = connect(topic, () => ({ problems: [{ kind: 'error', message: 'late', line: 1 }], delayMs: 1500 }))
  await Promise.all([a.opened, b.opened])
  const from = a.frames.length
  a.send(request(python(`ui_render(title="Two", html=${py(H)})`)))
  const verified = await verifiedSince(a, from)
  check('the first report is the outcome', verified.ui_page_verified.verification?.state === 'clean', verified)
  check('both tabs were asked', b.frames.some((f) => kind(f, 'ui_verify')))
  a.close()
  b.close()
}

// §4 ---------------------------------------------------------------------------------------
current = '§4 state across versions'
let pageId
let stateTopic
{
  stateTopic = fresh()
  const a = connect(stateTopic, () => ({ problems: [] }))
  const b = connect(stateTopic)
  await Promise.all([a.opened, b.opened])
  const drawn = a.frames.length
  a.send(request(python(`ui_render(title="Counter", html=${py(H)})`)))
  pageId = (await verifiedSince(a, drawn)).ui_page_verified.page_id

  const from = b.frames.length
  a.send({ ui_page_state: { page_id: pageId, state: { n: 1 } } })
  const shared = await b.next((f) => kind(f, 'ui_page_state'), 5000, from)
  check('the other tab gets the state', shared.ui_page_state.state?.n === 1 && shared.ui_page_state.state_seq >= 1, shared)

  const edited = a.frames.length
  a.send(request(python(`ui_render(page=${py(pageId)}, base_version=1, edits=[{"find": "Hi", "replace": "Count"}])`)))
  await a.next((f) => kind(f, 'ui_page_verified') && f.ui_page_verified.version === 2, 15000, edited)
  const page = await api(`${stateTopic}/pages/${pageId}`)
  check('the state is shared by the new version', page.state?.n === 1 && page.latest_version === 2, page)

  await sleep(1100)
  const burst = a.frames.length
  for (let i = 0; i < 5; i++) a.send({ ui_page_state: { page_id: pageId, state: { n: 10 + i } } })
  await sleep(800)
  const limited = a.frames.slice(burst).filter((f) => kind(f, 'client_error') && f.client_error.code === 'rate_limited')
  check('a burst is rate limited', limited.length >= 3, a.frames.slice(burst))

  const other = connect(fresh())
  await other.opened
  const at = other.frames.length
  other.send({ ui_page_state: { page_id: pageId, state: { n: 99 } } })
  const refused = await other.next((f) => kind(f, 'client_error'), 5000, at)
  check("another conversation's page is refused", refused.client_error.code === 'not_found', refused)
  other.close()
  a.close()
  b.close()
}

// §5 ---------------------------------------------------------------------------------------
current = '§5 talk back from a page'
{
  const s = connect(stateTopic)
  await s.opened
  const event = async (page) => {
    const from = s.frames.length
    s.send(request({ client_event: { text: 'e2e4', data: { from: 'e2', to: 'e4' }, page } }))
    await s.next(isReply, 15000, from)
    const entries = await history(stateTopic)
    return entries.filter((e) => e.content?.Request?.content?.client_event).at(-1)?.content.Request.content.client_event
  }
  const own = await event({ page_id: pageId, version: 2 })
  check('the event starts a turn and keeps its page', own?.page?.page_id === pageId && own.page.version === 2, own)
  const unknown = await event({ page_id: pageId, version: 9 })
  check('a version the page does not have is dropped', unknown && unknown.page === undefined, unknown)
  const foreign = await event({ page_id: 'pg_nothere', version: 1 })
  check('a page outside the conversation is dropped', foreign && foreign.page === undefined, foreign)
  s.close()
}

console.log('done')
process.exit(0)
