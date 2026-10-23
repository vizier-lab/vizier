#!/usr/bin/env node
// Seeds a running vizier with the scale quickstart §6 asks for: 50 agents, 300 tasks, 1,000
// memory bundles, 600 conversations and 1,000 roles, all through the HTTP API. The agents run on
// the offline `dummyplug` provider, so nothing needs a key.
//
//   VIZIER_URL=http://localhost:9999 VIZIER_USER=admin VIZIER_PASSWORD=admin node scripts/seed-scale.mjs
//
// Dependency-free (Node >= 22). Safe to run again: what already exists is skipped.

const BASE = (process.env.VIZIER_URL ?? 'http://localhost:9999').replace(/\/$/, '')
const API = `${BASE}/api/v1`
const USER = process.env.VIZIER_USER ?? 'admin'
const PASSWORD = process.env.VIZIER_PASSWORD ?? 'admin'
const COUNTS = {
    agents: Number(process.env.SEED_AGENTS ?? 50),
    tasks: Number(process.env.SEED_TASKS ?? 300),
    bundles: Number(process.env.SEED_BUNDLES ?? 1000),
    conversations: Number(process.env.SEED_CONVERSATIONS ?? 600),
    roles: Number(process.env.SEED_ROLES ?? 1000),
}
const CONCURRENCY = Number(process.env.SEED_CONCURRENCY ?? 8)

let token = ''

async function call(method, path, body) {
    const res = await fetch(`${API}${path}`, {
        method,
        headers: { 'content-type': 'application/json', ...(token ? { authorization: `Bearer ${token}` } : {}) },
        body: body === undefined ? undefined : JSON.stringify(body),
    })
    const json = await res.json().catch(() => ({}))
    return { ok: res.ok, status: res.status, data: json.data, message: json.message }
}

/** Runs `work(i)` for 0..n-1, `CONCURRENCY` at a time, reporting progress. */
async function each(label, n, work) {
    let next = 0
    let done = 0
    let skipped = 0
    const started = Date.now()
    const worker = async () => {
        while (next < n) {
            const i = next++
            const result = await work(i)
            if (result === 'skipped') skipped++
            if (++done % 100 === 0 || done === n) {
                process.stdout.write(`\r${label}: ${done}/${n}${skipped ? ` (${skipped} existed)` : ''}   `)
            }
        }
    }
    await Promise.all(Array.from({ length: CONCURRENCY }, worker))
    console.log(`\r${label}: ${n}${skipped ? ` (${skipped} existed)` : ''} in ${((Date.now() - started) / 1000).toFixed(1)}s   `)
}

const pad = (n, w = 4) => String(n).padStart(w, '0')
const agentId = (i) => `seed-${pad(i, 3)}`

const login = await call('POST', '/auth/login', { username: USER, password: PASSWORD })
if (!login.ok) {
    console.error(`sign-in failed (${login.status}): ${login.message ?? ''}`)
    process.exit(1)
}
token = login.data.token

await each('agents', COUNTS.agents, async (i) => {
    const res = await call('POST', '/agents', {
        agent_id: agentId(i),
        name: `Seed agent ${pad(i, 3)}`,
        description: `Seeded for the scale checks (${i})`,
        provider: 'dummyplug',
        model: 'dummyplug',
    })
    if (!res.ok && res.status !== 400 && res.status !== 409) throw new Error(`agent ${i}: ${res.status} ${res.message}`)
    return res.ok ? 'created' : 'skipped'
})

// Tasks, bundles and conversations hang off a handful of agents, as real ones tend to.
const HOSTS = Math.min(COUNTS.agents, 5)
const host = (i) => agentId(i % HOSTS)

await each('tasks', COUNTS.tasks, async (i) => {
    const res = await call('POST', `/agents/${host(i)}/tasks`, {
        slug: `seed-task-${pad(i)}`,
        title: `Seed task ${pad(i)}`,
        instruction: 'Say hello.',
        schedule: { type: 'Cron', expression: `${i % 60} ${i % 24} * * *` },
        is_active: i % 3 !== 0,
    })
    if (!res.ok && res.status !== 409 && res.status !== 400) throw new Error(`task ${i}: ${res.status} ${res.message}`)
    return res.ok ? 'created' : 'skipped'
})

await each('bundles', COUNTS.bundles, async (i) => {
    const res = await call('POST', `/agents/${host(i)}/memory`, {
        title: `Seed note ${pad(i)}`,
        content: `A seeded document in bundle ${pad(i)}.`,
        bundle: `seed-bundle-${pad(i)}`,
        tags: ['seed'],
    })
    if (!res.ok && res.status !== 409 && res.status !== 400) throw new Error(`bundle ${i}: ${res.status} ${res.message}`)
    return res.ok ? 'created' : 'skipped'
})

await each('conversations', COUNTS.conversations, async (i) => {
    // Each channel id is its own conversation for the signed-in person.
    const res = await call('POST', `/agents/${agentId(0)}/chat`, {
        channel_id: `seed-${pad(i)}`,
        topic_id: 'topic-1',
        content: `Seed conversation ${pad(i)}`,
    })
    if (!res.ok) throw new Error(`conversation ${i}: ${res.status} ${res.message}`)
})

const PERMISSIONS = (await call('GET', '/auth/roles/available-permissions')).data?.permissions ?? []
await each('roles', COUNTS.roles, async (i) => {
    const picked = PERMISSIONS.filter((_, p) => (i >> p) & 1)
    const res = await call('POST', '/auth/roles', { name: `seed-role-${pad(i)}`, permissions: picked })
    if (!res.ok && res.status !== 409 && res.status !== 400) throw new Error(`role ${i}: ${res.status} ${res.message}`)
    return res.ok ? 'created' : 'skipped'
})

console.log('done')
