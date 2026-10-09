import fs from 'fs'
const token = fs.readFileSync('token', 'utf8').trim()
async function convo(agent, topic, msgs) {
  const ws = new WebSocket(`ws://localhost:9911/api/v1/agents/${agent}/channel/vizier-webui/topic/${topic}/chat?token=${token}`)
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j })
  for (const m of msgs) {
    const done = new Promise((resolve) => {
      ws.onmessage = (ev) => {
        const d = JSON.parse(ev.data)
        const c = d.content
        if (typeof c === 'object' && ('message' in c || 'error' in c || 'checkpoint' in c)) resolve(c)
        if (c === 'empty' || c === 'abort') resolve(c)
      }
    })
    ws.send(JSON.stringify({ timestamp: new Date().toISOString(), user: 'admin', content: m.command ? { command: m.command } : { chat: m }, metadata: null }))
    const res = await Promise.race([done, new Promise(r => setTimeout(() => r('timeout'), 20000))])
    console.log(topic, JSON.stringify(m).slice(0, 60), '->', JSON.stringify(res).slice(0, 150))
  }
  ws.close()
}
await convo('aria', 'General', [
  'Hi Aria! What can you do?',
  'tools',
  '{"tool":"READ_CORE","arguments":{}}',
  '{"tool":"memory_search","arguments":{"query":"rust async"}}',
])
await convo('aria', 'ui-rewrite', [
  'Let us plan the UI rewrite',
  { command: 'checkpoint' },
  'memory_read',
])
