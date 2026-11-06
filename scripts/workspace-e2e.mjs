#!/usr/bin/env node
// End-to-end check of the Workspace channel (specs/053-workspace-channel/quickstart.md §1 to §6)
// against a running vizier with agents on the offline `dummyplug` provider.
//
//   dummyplug:  cd dummyplug && node src/server.mjs        (listens on :9998)
//   vizier:     just run                                   (or any server on VIZIER_URL)
//   this:       VIZIER_URL=http://localhost:9999 VIZIER_TOKEN=<jwt> node scripts/workspace-e2e.mjs
//
// A fresh server can be set up by the script itself: VIZIER_SETUP_USER / VIZIER_SETUP_PASSWORD
// create the first user. It creates the agents `alpha` and `beta` when they are missing.
//
// The checks that need a second or third person (a member who cannot manage, a stranger who gets
// 404) are pinned in crates/vizier-core/tests/workspaces_service.rs. Dependency-free: Node 22+.

const BASE = (process.env.VIZIER_URL ?? 'http://localhost:9999').replace(/\/$/, '')
const WS = BASE.replace(/^http/, 'ws')
let TOKEN = process.env.VIZIER_TOKEN

let current = 'setup'
function check(name, ok, detail) {
  if (!ok) {
    console.error(`FAIL ${current} › ${name}${detail === undefined ? '' : `\n  ${typeof detail === 'string' ? detail : JSON.stringify(detail).slice(0, 2000)}`}`)
    process.exit(1)
  }
  console.log(`  ok ${name}`)
}
const section = (name) => {
  current = name
  console.log(name)
}
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

async function api(method, path, body) {
  const res = await fetch(`${BASE}/api/v1${path}`, {
    method,
    headers: { 'content-type': 'application/json', ...(TOKEN ? { authorization: `Bearer ${TOKEN}` } : {}) },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  const json = await res.json().catch(() => ({}))
  return { status: res.status, data: json.data, body: json }
}

/** `{ "tool": "execute_python", ... }` as dummyplug takes it: agents are in code mode, so every
 * tool is reached through a script. `say` is the text written beside the call (a mid-turn message). */
const py = (code, say) => JSON.stringify({ ...(say ? { say } : {}), tool: 'execute_python', arguments: { intent: 'e2e', code } })

/** A socket on one topic that keeps every frame it sees. */
function watch(workspaceId, topicId) {
  const ws = new WebSocket(`${WS}/api/v1/workspaces/${workspaceId}/topics/${topicId}/stream?token=${TOKEN}`)
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
  const entries = () => frames.flatMap((f) => (f.workspace_entry ? [f.workspace_entry] : []))
  return {
    opened,
    frames,
    entries,
    close: () => ws.close(),
    /** Waits for an entry matching `predicate`, new since `after` (a count of entries seen). */
    async entry(predicate, after = 0, ms = 20_000) {
      const start = Date.now()
      while (Date.now() - start < ms) {
        const found = entries().slice(after).find(predicate)
        if (found) return found
        await sleep(100)
      }
      return null
    },
  }
}

// --- setup ----------------------------------------------------------------------------------------
if (!TOKEN && process.env.VIZIER_SETUP_USER) {
  const r = await api('POST', '/auth/setup', { username: process.env.VIZIER_SETUP_USER, password: process.env.VIZIER_SETUP_PASSWORD ?? 'e2e-password' })
  TOKEN = r.data?.token
}
if (!TOKEN) {
  console.error('VIZIER_TOKEN (or VIZIER_SETUP_USER on a fresh server) is required')
  process.exit(2)
}
for (const id of ['alpha', 'beta']) {
  const have = await api('GET', `/agents/${id}`)
  if (have.status !== 200) {
    const made = await api('POST', '/agents', { agent_id: id, name: id[0].toUpperCase() + id.slice(1), provider: 'dummyplug', model: 'dummyplug' })
    check(`agent ${id} created`, made.status < 300, made.body)
  }
}
await sleep(1500) // the agents start in the background

const NAME = `e2e-${Date.now().toString(36)}`

// --- §1 membership --------------------------------------------------------------------------------
section('§1 membership')
const made = await api('POST', '/workspaces', { name: NAME })
check('a Workspace is created with a first topic', made.status === 201 && made.data.topics.length === 1 && made.data.topics[0].name === 'general', made.body)
const wid = made.data.id
const tid = made.data.topics[0].id
const dup = await api('POST', '/workspaces', { name: NAME.toUpperCase() })
check('the same name in capitals is refused', dup.status === 409 && dup.body.data?.error === 'name_in_use', dup.body)
check('a bad name is refused', (await api('POST', '/workspaces', { name: 'Not Allowed!' })).status === 422)
for (const id of ['alpha', 'beta']) {
  const added = await api('POST', `/workspaces/${wid}/members`, { kind: 'agent', handle: id })
  check(`${id} is added`, added.status === 201, added.body)
}
const twice = await api('POST', `/workspaces/${wid}/members`, { kind: 'agent', handle: 'alpha' })
check('a member cannot be added twice', twice.status === 409, twice.body)
check('a missing Workspace is a 404', (await api('GET', '/workspaces/missing')).status === 404)

const live = watch(wid, tid)
await live.opened
await sleep(300)
check('the socket starts with a hello', live.frames[0]?.hello !== undefined, live.frames)

// --- §2 silent reading ----------------------------------------------------------------------------
section('§2 silent reading costs nothing')
for (const text of ['the billing lock is back', 'it was the deploy', 'rolling back now']) {
  const r = await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text })
  check(`posted "${text}"`, r.status === 201 && r.data.entry, r.body)
}
await sleep(1500)
check('three entries reached the socket', live.entries().filter((e) => e.kind === 'person').length === 3)
check('no agent replied', live.entries().every((e) => e.kind !== 'agent'))
const quiet = await api('GET', `/workspaces/${wid}/topics/${tid}/entries`)
check('no agent took a step', quiet.data.steps.length === 0, quiet.data.steps)

// --- §3 a mention wakes one agent ------------------------------------------------------------------
section('§3 a mention wakes one agent')
let seen = live.entries().length
await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: '@alpha tools' })
const alphaReply = await live.entry((e) => e.kind === 'agent' && e.author.id === 'alpha' && e.is_final, seen)
check('alpha answers', !!alphaReply)
check('it answered what was asked once the mention was taken out', alphaReply.body.includes('Available tools'), alphaReply.body)
check('beta did not wake', !live.entries().some((e) => e.author.id === 'beta'))

// --- §4 several at once, and what does not count --------------------------------------------------------
section('§4 several at once')
seen = live.entries().length
await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: '@alpha @beta tools' })
check('alpha answers', !!(await live.entry((e) => e.author.id === 'alpha' && e.is_final, seen)))
check('beta answers', !!(await live.entry((e) => e.author.id === 'beta' && e.is_final, seen)))
seen = live.entries().length
await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: '@alpha @alpha tools' })
await live.entry((e) => e.author.id === 'alpha' && e.is_final, seen)
await sleep(1000)
check('naming an agent twice wakes it once', live.entries().slice(seen).filter((e) => e.author.id === 'alpha' && e.is_final).length === 1)
seen = live.entries().length
await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: '`@alpha` is just code here\n> @beta quoted\nand @nobody is nobody' })
await sleep(1500)
check('code, a quote and a stranger wake no one', live.entries().slice(seen).every((e) => e.kind === 'person'))

// --- §5 a mention in an agent's own words ---------------------------------------------------------------
section('§5 an agent wakes another')
// The person's own message carries the JSON, and the JSON contains `@beta`, so it names beta too.
// What this checks is that alpha's mid-turn text is a message of its own whose mention reads woken.
seen = live.entries().length
await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: `@alpha ${py('1 + 1', 'asking @beta now')}` })
const said = await live.entry((e) => e.author.id === 'alpha' && e.body.includes('asking @beta'), seen)
check('alpha says it mid-turn', !!said)
check('beta is woken by it', !!(await live.entry((e) => e.author.id === 'beta' && e.is_final, seen)))
const after = await api('GET', `/workspaces/${wid}/topics/${tid}/entries`)
const mention = after.data.entries.find((e) => e.seq === said.seq)?.mentions.find((m) => m.handle === 'beta')
check('the mention reads woken', mention?.state === 'woken', mention)

// --- §5a the tool, called from a script ---------------------------------------------------------------
section('§5a mention_agent from a script')
seen = live.entries().length
await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: `@alpha ${py('mention_agent(agents=["beta"], message="tools")')}` })
const viaTool = await live.entry((e) => e.author.id === 'alpha' && e.body.includes('@beta tools') && e.history_uid === null, seen)
check('alpha posts the mention as a message with no history row of its own', !!viaTool, live.entries().slice(seen))
check('beta answers it', !!(await live.entry((e) => e.author.id === 'beta' && e.is_final && e.body.includes('Available tools'), seen)))
seen = live.entries().length
await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: `@alpha ${py('mention_agent(agents=["stranger"], message="hi")')}` })
const refused = await live.entry((e) => e.author.id === 'alpha' && e.is_final, seen)
check('a non-member is reported, not posted', refused?.body.includes('not_a_member'), refused?.body)
check('and nothing was posted to them', !live.entries().slice(seen).some((e) => e.body.includes('@stranger')))

// --- §8 reaching a Workspace from elsewhere --------------------------------------------------------------
section('§8 reaching a Workspace')
seen = live.entries().length
await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: `@alpha ${py(`list_workspaces()`)}` })
const listed = await live.entry((e) => e.author.id === 'alpha' && e.is_final, seen)
check('list_workspaces names this Workspace', listed?.body.includes(NAME), listed?.body)
seen = live.entries().length
await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: `@alpha ${py(`send_message_to_workspace(workspace_name="${NAME}", workspace_topic="daily", message="@beta tools")`)}` })
const daily = await live.entry((e) => e.author.id === 'beta' && e.is_final, seen, 25_000)
check('beta answers in the new topic', !!daily)
const topics = await api('GET', `/workspaces/${wid}`)
check('the topic was created', topics.data.topics.some((t) => t.name === 'daily'), topics.data.topics)
const other = await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: `@alpha ${py('send_message_to_workspace(workspace_name="no-such", workspace_topic="x", message="hi")')}` })
check('posting works', other.status === 201)

// --- §9 the old tools are gone -----------------------------------------------------------------------------
section('§9 the old tools are gone')
await sleep(2500) // let the §8 turn finish so its closing message is not taken for this one
seen = live.entries().length
await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: `@alpha ${py('consult_agent(agent_id="beta", prompt="hi")')}` })
const retired = await live.entry((e) => e.author.id === 'alpha' && e.is_final, seen)
check('a script calling consult_agent gets the new guidance', retired?.body.includes('mention_agent'), retired?.body)

// --- §5b commands -------------------------------------------------------------------------------------
section('§5b skills and commands')
const btw = await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: '/btw @alpha hello' })
check('/btw is refused and posts nothing', btw.status === 422 && btw.body.data?.error === 'not_in_workspace', btw.body)
const clear = await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: '/clear-files' })
check('/clear-files is refused', clear.status === 422, clear.body)
seen = live.entries().length
const skills = await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: '/skills @alpha' })
check('/skills @alpha is accepted', skills.status === 201, skills.body)
check('alpha lists its skills', !!(await live.entry((e) => e.author.id === 'alpha' && e.is_final, seen)))
const menu = await api('GET', `/workspaces/${wid}/commands`)
check('the / menu lists commands', menu.status === 200 && menu.data.commands.some((c) => c.name === 'abort'), menu.body)
seen = live.entries().length
const note = await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: '/checkpoint @alpha' })
check('/checkpoint answers a note and no message', note.status === 201 && !note.data.entry && note.data.note?.kind === 'note', note.body)

// --- §6 stop all ------------------------------------------------------------------------------------------
section('§6 stop all')
const stop = await api('POST', `/workspaces/${wid}/stop`)
check('stop all answers counts', stop.status === 200 && typeof stop.data.stopped === 'number' && typeof stop.data.dropped === 'number', stop.body)
seen = live.entries().length
await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: '@alpha tools' })
check('an agent answers again after a stop', !!(await live.entry((e) => e.author.id === 'alpha' && e.is_final, seen)))

// --- files and reactions -------------------------------------------------------------------------------
section('files and reactions')
const uploaded = await api('POST', '/files/upload', { file: Buffer.from('hello').toString('base64'), filename: 'notes.txt' })
check('a file uploads', uploaded.status === 201, uploaded.body)
seen = live.entries().length
const withFile = await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: '@alpha context', files: [uploaded.data.file_id] })
check('a message carries it', withFile.status === 201 && withFile.data.entry.files[0]?.name === 'notes.txt', withFile.body)
const fileReply = await live.entry((e) => e.author.id === 'alpha' && e.is_final, seen)
check('the woken agent answers', !!fileReply)
const alone = await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: '', files: [uploaded.data.file_id] })
check('a message can be files alone', alone.status === 201, alone.body)
const noFile = await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: 'x', files: ['nope'] })
check('an id nobody uploaded is refused', noFile.status === 422 && noFile.body.data?.error === 'unknown_file', noFile.body)
const reacted = await api('POST', `/workspaces/${wid}/topics/${tid}/reactions`, { message_uid: fileReply.history_uid, emoji: '👍', action: 'added' })
check('a reply takes a reaction', reacted.status === 200, reacted.body)
const strayReaction = await api('POST', `/workspaces/${wid}/topics/${tid}/reactions`, { message_uid: 'nope', emoji: '👍', action: 'added' })
check('a message outside the topic is a 404', strayReaction.status === 404, strayReaction.body)

// A bare newline beside a call says nothing: no message, and no entry for anyone to read.
seen = live.entries().length
await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: `@alpha ${py('1', '\n\n')}` })
await live.entry((e) => e.author.id === 'alpha' && e.is_final, seen)
await sleep(300)
const page = await api('GET', `/workspaces/${wid}/topics/${tid}/entries?limit=200`)
check('blank narration is not saved as a step', !page.data.steps.some((s) => typeof s.entry.content.AssistantMessage === 'string' && !s.entry.content.AssistantMessage.trim()), page.data.steps.length)
check('nor shown as a message', !live.entries().slice(seen).some((e) => e.kind === 'agent' && !e.body.trim()))

// --- §10 notifications ------------------------------------------------------------------------------------
// One person is signed in, so this checks the agent side: the owner is told about every message an
// agent wrote and about none of their own. Who else is told is pinned in
// crates/vizier-core/tests/workspaces_service.rs.
section('§10 every message notifies the others')
await sleep(500)
const agentMessages = live.entries().filter((e) => e.kind === 'agent')
const inbox = await api('GET', '/notifications?filter=all&limit=50')
const rows = (inbox.data?.items ?? []).filter((n) => n.category === 'workspace_message')
// Other topics (§8's `daily`) also notify, and their entries are not on this socket, so this is a floor.
check('the owner has a row for each agent message', rows.length >= Math.min(agentMessages.length, 50), { rows: rows.length, agent: agentMessages.length })
check('and none for their own', !rows.some((n) => n.text.startsWith('e2e in ')), rows.map((n) => n.text))
check('each links to the topic', rows.every((n) => n.href.startsWith(`/workspaces/${wid}/`)), rows[0])
const unreadBefore = (await api('GET', '/workspaces')).data.find((w) => w.id === wid)
check('the Workspace shows them as unread', unreadBefore.unread === rows.length, unreadBefore)
const read = await api('POST', `/workspaces/${wid}/topics/${tid}/read`)
check('opening the topic reads them', read.status === 200 && read.data.marked > 0, read.body)
const unreadAfter = (await api('GET', '/workspaces')).data.find((w) => w.id === wid)
check('its topic has none left unread', unreadAfter.topics.find((t) => t.id === tid).unread === 0, unreadAfter)

// --- usage ------------------------------------------------------------------------------------------
section('usage names the Workspace and topic')
await sleep(1000)
const usage = await api('GET', '/agents/alpha/usage')
check('the report answers', usage.status === 200, usage.body)
const rowsByPlace = usage.data.by_place ?? []
check('Workspace activity is counted', (usage.data.by_activity ?? []).some((r) => r.key === 'workspace' && r.calls > 0), usage.data.by_activity)
check('its place is named Workspace / #topic', rowsByPlace.some((r) => r.label.includes(' / #')), rowsByPlace)

// --- §11 managing a Workspace ------------------------------------------------------------------------
section('§11 managing a Workspace')
const found = await api('GET', `/workspaces/${wid}/candidates?q=bet`)
check('search finds beta, already a member', found.status === 200 && found.data.agents.items.some((a) => a.id === 'beta' && a.member), found.body)
check('search needs some text', (await api('GET', `/workspaces/${wid}/candidates?q=`)).status >= 400)
const scratchTopic = await api('POST', `/workspaces/${wid}/topics`, { name: 'scratch' })
check('a topic is added', scratchTopic.status === 201, scratchTopic.body)
const clash = await api('PATCH', `/workspaces/${wid}/topics/${scratchTopic.data.id}`, { name: 'GENERAL' })
check('renaming onto an existing name is refused', clash.status === 409, clash.body)
const renamedTopic = await api('PATCH', `/workspaces/${wid}/topics/${scratchTopic.data.id}`, { name: 'scratch-2' })
check('a topic is renamed', renamedTopic.status === 200 && renamedTopic.data.name === 'scratch-2', renamedTopic.body)
check('a topic is deleted', (await api('DELETE', `/workspaces/${wid}/topics/${scratchTopic.data.id}`)).status === 200)
const detailNow = (await api('GET', `/workspaces/${wid}`)).data
const onlyTopic = detailNow.topics.length === 1 ? detailNow.topics[0].id : null
if (onlyTopic) check('the last topic stays', (await api('DELETE', `/workspaces/${wid}/topics/${onlyTopic}`)).status === 409)
const owner = detailNow.members.find((m) => m.kind === 'person' && m.role === 'owner')
const last = await api('PATCH', `/workspaces/${wid}/members/person/${owner.handle}`, { role: 'member' })
check('the last owner cannot step down', last.status === 409, last.body)

// Removing an agent ends its turn at once (SC-006). The sandbox has no sleep, so the script counts:
// long enough that the turn is still running when the removal lands.
seen = live.entries().length
await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: `@alpha ${py('x = 0\nfor i in range(150000000):\n    x += i\nx')}` })
await sleep(800)
const running = (await api('GET', '/workspaces')).data.find((w) => w.id === wid)
check('the turn is running when the agent is removed', running.working === true, running)
const gone = await api('DELETE', `/workspaces/${wid}/members/agent/alpha`)
check('an agent is removed', gone.status === 200, gone.body)
await sleep(9000)
const members = (await api('GET', `/workspaces/${wid}`)).data.members
check('it is no longer a member', !members.some((m) => m.handle === 'alpha'))
check('its turn wrote no reply', !live.entries().slice(seen).some((e) => e.author.id === 'alpha' && e.is_final), live.entries().slice(seen).map((e) => e.body.slice(0, 60)))
const afterRemoval = (await api('GET', '/workspaces')).data.find((w) => w.id === wid)
check('and nothing is working any more', afterRemoval.working === false, afterRemoval)
await api('POST', `/workspaces/${wid}/topics/${tid}/entries`, { text: '@alpha are you there' })
await sleep(1500)
check('and it does not wake', !live.entries().slice(seen).some((e) => e.kind === 'agent' && e.author.id === 'alpha'))

const renamed = await api('PATCH', `/workspaces/${wid}`, { name: `${NAME}-b` })
check('the Workspace is renamed', renamed.status === 200 && renamed.data.name === `${NAME}-b`, renamed.body)
check('it is deleted', (await api('DELETE', `/workspaces/${wid}`)).status === 200)
check('and is gone', (await api('GET', `/workspaces/${wid}`)).status === 404)

live.close()
console.log('\nAll Workspace checks passed.')
