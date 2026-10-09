// Seeds a fresh vizier instance with demo data via the REST API.
import fs from 'fs'
const BASE = process.env.BASE || 'http://localhost:9911/api/v1'
let token = null

async function call(method, path, body) {
  const res = await fetch(BASE + path, {
    method,
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  })
  const text = await res.text()
  let json
  try { json = JSON.parse(text) } catch { json = text }
  if (!res.ok) console.error(`!! ${method} ${path} -> ${res.status}`, text.slice(0, 300))
  return json
}

const setup = await call('GET', '/auth/setup-status')
if (setup.data?.needs_setup) {
  const r = await call('POST', '/auth/setup', { username: 'admin', password: 'demo-password' })
  token = r.data.token
} else {
  const r = await call('POST', '/auth/login', { username: 'admin', password: 'demo-password' })
  token = r.data.token
}

// Providers (placeholder credentials only)
await call('PUT', '/providers/ollama', { base_url: 'http://localhost:11434', enabled: true })
await call('PUT', '/providers/openai', { api_key: 'sk-demo-not-a-real-key', enabled: true })
await call('PUT', '/providers/dummyplug', { enabled: true })

const agentBase = {
  provider: 'dummyplug',
  model: 'dummyplug',
  thinking_depth: 0,
  checkpoint_threshold: 0.8,
  tools: { fetch: true, http_client: true, timeout: '30m' },
  prompt_timeout: '60m',
  dream_enabled: true,
  dream_schedule: '0 2 * * *',
}
await call('POST', '/agents', {
  ...agentBase,
  agent_id: 'aria',
  name: 'Aria',
  description: 'Personal research assistant and note keeper',
  system_prompt: 'You are Aria, a careful research assistant. Cite sources and keep notes in memory.',
})
await call('POST', '/agents', {
  ...agentBase,
  agent_id: 'scribe',
  name: 'Scribe',
  description: 'Drafts weekly reports and summaries',
  system_prompt: 'You are Scribe. You write concise weekly summaries.',
})
await call('POST', '/agents', {
  ...agentBase,
  agent_id: 'ops-bot',
  name: 'Ops Bot',
  description: 'Watches deployments and pings on-call',
  dream_enabled: false,
})

// Memories across bundles, with links between them
const mem = (bundle, path, title, content, tags) =>
  call('POST', '/agents/aria/memory', { bundle, path, title, content, tags })
await mem('default', 'user-preferences', 'User preferences',
  '# User preferences\n\n- Prefers concise answers with sources\n- Works in UTC+7\n- See [current projects](projects.md) and [[books/dune]]', ['user', 'profile'])
await mem('default', 'projects', 'Current projects',
  '# Current projects\n\n1. **Vizier UI rewrite** — inventory first\n2. Home lab migration\n\nRelated: [preferences](user-preferences.md), [[research/rust-async]]', ['projects'])
await mem('default', 'meeting-notes/2026-10-01', 'Weekly sync 2026-10-01',
  '## Weekly sync\n\n- Agreed to freeze the current UI\n- Next: capture every page\n\nSee [projects](../projects.md)', ['meeting'])
await mem('research', 'rust-async', 'Rust async runtimes',
  '# Rust async runtimes\n\nTokio is the default. Notes on `spawn_blocking`, cancellation and structured concurrency.\n\nSee [[research/sqlite-vec]]', ['rust', 'async'])
await mem('research', 'sqlite-vec', 'sqlite-vec notes',
  '# sqlite-vec\n\nvec0 virtual tables, one row per passage.\n\nBack to [rust async](rust-async.md)', ['sqlite', 'vector'])
await mem('books', 'dune', 'Dune (Frank Herbert)',
  '# Dune\n\n> Fear is the mind-killer.\n\nRead in 2025. Rating: 5/5', ['books', 'scifi'])
await mem('books', 'neuromancer', 'Neuromancer',
  '# Neuromancer\n\nCyberpunk classic. Pairs with [Dune](dune.md) on the reading list.', ['books', 'scifi'])
// Edit one memory so it has version history
await call('PUT', '/agents/aria/memory/user-preferences?bundle=default', {
  title: 'User preferences',
  content: '# User preferences\n\n- Prefers concise answers with sources\n- Works in UTC+7\n- Likes tables for comparisons\n- See [current projects](projects.md) and [[books/dune]]',
  bundle: 'default',
  tags: ['user', 'profile'],
})

// CORE: write twice so history has entries
await call('PUT', '/agents/aria/core', { content: '# CORE\n\nI am **Aria**, a research assistant.\n\n## Principles\n- Be accurate\n- Keep notes in memory\n' })
await call('PUT', '/agents/aria/core', { content: '# CORE\n\nI am **Aria**, a research assistant for the Vizier team.\n\n## Principles\n- Be accurate, cite sources\n- Keep notes in memory\n- Ask before scheduling recurring tasks\n' })

// Tasks
await call('POST', '/agents/aria/tasks', {
  slug: 'morning-briefing', title: 'Morning briefing',
  instruction: 'Summarise overnight news on **Rust** and **AI agents** and post it to the General topic.',
  schedule: { type: 'Cron', expression: '0 9 * * 1-5' },
})
await call('POST', '/agents/aria/tasks', {
  slug: 'weekly-review', title: 'Weekly review',
  instruction: 'Review this week\'s memories and write a short summary.',
  schedule: { type: 'Cron', expression: '0 18 * * 0' },
})
await call('POST', '/agents/aria/tasks', {
  slug: 'renew-domain', title: 'Remind me to renew the domain',
  instruction: 'Remind the user to renew vizier.dev.',
  schedule: { type: 'OneTime', datetime: '2026-12-01T09:00:00Z' },
})

// Skills
await call('POST', '/agents/aria/skills', {
  name: 'code-review', description: 'Review a diff for correctness and style',
  content: '# Code review\n\n1. Read the diff\n2. Check for bugs\n3. Suggest simplifications', keywords: ['review', 'quality'],
})
await call('POST', '/agents/aria/skills', {
  name: 'paper-summary', description: 'Summarise an academic paper into key findings',
  content: '# Paper summary\n\n- Problem\n- Method\n- Results\n- Limitations', keywords: ['research', 'summary'],
})

// Users / roles / api keys
const role = await call('POST', '/auth/roles', { name: 'viewer', permissions: ['owned_agents:view', 'settings:password'] })
await call('POST', '/auth/users', { username: 'alice', password: 'demo-password', role_id: role.data?.role_id ?? role.data?.id })
await call('POST', '/auth/users', { username: 'bob', password: 'demo-password' })
await call('POST', '/auth/api-keys', { name: 'CI pipeline', expires_in_days: 90 })

fs.writeFileSync("token", token)
console.log("seeded; token written to ./token")
