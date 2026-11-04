#!/usr/bin/env node

const base = (process.env.VIZIER_URL ?? 'http://127.0.0.1:9999').replace(/\/$/, '')
const user = process.env.VIZIER_SMOKE_USER ?? 'kubernetes-smoke'
const password = process.env.VIZIER_SMOKE_PASSWORD ?? 'kubernetes-smoke-password'
const agent = process.env.VIZIER_SMOKE_AGENT ?? 'kubernetes-smoke'
const topic = process.env.VIZIER_SMOKE_TOPIC ?? 'persistence'
const verify = process.argv.includes('--verify')

async function request(path, options = {}) {
  const response = await fetch(`${base}${path}`, options)
  const body = await response.json().catch(() => ({}))
  if (!response.ok) throw new Error(`${options.method ?? 'GET'} ${path}: ${response.status} ${JSON.stringify(body)}`)
  return body.data ?? body
}

async function token() {
  const status = await request('/api/v1/auth/setup-status')
  if (status.needs_setup) {
    return (await request('/api/v1/auth/setup', {
      method: 'POST', headers: {'content-type': 'application/json'}, body: JSON.stringify({username: user, password}),
    })).token
  }
  return (await request('/api/v1/auth/login', {
    method: 'POST', headers: {'content-type': 'application/json'}, body: JSON.stringify({username: user, password}),
  })).token
}

const jwt = await token()
const headers = {authorization: `Bearer ${jwt}`, 'content-type': 'application/json'}
if (!verify && process.env.DUMMYPLUG_URL) {
  await request('/api/v1/providers/dummyplug', {
    method: 'PUT',
    headers,
    body: JSON.stringify({base_url: process.env.DUMMYPLUG_URL}),
  })
}

if (!verify) {
  const created = await fetch(`${base}/api/v1/agents`, {
    method: 'POST', headers, body: JSON.stringify({agent_id: agent, name: 'Kubernetes smoke', description: 'Persistence and WebSocket smoke', provider: 'dummyplug', model: 'dummyplug'}),
  })
  if (!created.ok && created.status !== 409) throw new Error(`create agent: ${created.status} ${await created.text()}`)

  const wsBase = base.replace(/^http/, 'ws')
  const socket = new WebSocket(`${wsBase}/api/v1/agents/${agent}/channel/e2e/topic/${topic}/chat?token=${jwt}`)
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, {once: true})
    socket.addEventListener('error', reject, {once: true})
  })
  const frames = []
  const reply = new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error(`WebSocket reply timed out; frames=${JSON.stringify(frames)}`)),
      60000,
    )
    socket.addEventListener('message', (event) => {
      const frame = JSON.parse(event.data)
      frames.push(frame)
      const text = frame?.content?.message?.content
      if (typeof text === 'string') {
        clearTimeout(timer)
        resolve(text)
      }
    })
  })
  socket.send(JSON.stringify({timestamp: new Date().toISOString(), user, content: {chat: 'tools'}, metadata: {}}))
  const text = await reply
  if (!text.trim()) throw new Error('dummyplug returned an empty reply')
  socket.close()
  console.log('Authenticated dummyplug WebSocket turn passed')
} else {
  const history = await request(`/api/v1/agents/${agent}/channel/e2e/topic/${topic}/history?limit=100`, {headers})
  if (!Array.isArray(history) || history.length < 2) throw new Error(`persisted history missing: ${JSON.stringify(history)}`)
  console.log('Owner, agent, and dummyplug history survived pod replacement')
}
