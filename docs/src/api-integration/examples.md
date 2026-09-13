# 3.2 Examples

All examples assume Vizier on `http://localhost:9999`. Replace `assistant` with your agent id.

## curl

```sh
# First-run setup (only works while no user exists)
curl -s localhost:9999/api/v1/auth/setup -H 'content-type: application/json' \
  -d '{"username":"alice","password":"s3cret"}'

# Login → JWT
TOKEN=$(curl -s localhost:9999/api/v1/auth/login -H 'content-type: application/json' \
  -d '{"username":"alice","password":"s3cret"}' | jq -r .data.token)

# Add a provider key
curl -s -X PUT localhost:9999/api/v1/providers/openai \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"api_key":"sk-…"}'

# Create an agent
curl -s localhost:9999/api/v1/agents \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"agent_id":"assistant","name":"Assistant","provider":"openai","model":"gpt-4.1-mini","tools":{"fetch":true}}'

# Synchronous chat
curl -s localhost:9999/api/v1/agents/assistant/chat \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"channel_id":"cli","topic_id":"t1","content":"What is Vizier?"}' | jq .data.content

# Long-lived API key (use as: authorization: ApiKey vk_…)
curl -s localhost:9999/api/v1/auth/api-keys \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"name":"ci","expires_in_days":90}' | jq .data.key
```

## JavaScript

### Login + REST chat

```javascript
const base = 'http://localhost:9999/api/v1';

const login = await fetch(`${base}/auth/login`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ username: 'alice', password: 's3cret' }),
});
const { data: { token } } = await login.json();

const res = await fetch(`${base}/agents/assistant/chat`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
  body: JSON.stringify({ channel_id: 'my-app', topic_id: 'session-1', content: 'Hello!' }),
});
const { data } = await res.json();
console.log(data.content, data.stats?.total_tokens);
```

### WebSocket streaming

```javascript
const ws = new WebSocket(
  `ws://localhost:9999/api/v1/agents/assistant/channel/my-app/topic/session-1/chat?token=${token}`
);

ws.onopen = () => {
  ws.send(JSON.stringify({
    timestamp: new Date().toISOString(),
    user: 'alice',
    content: { chat: 'Summarize today\'s news about Rust.' },
    metadata: {},
    attachments: [],
  }));
};

ws.onmessage = (event) => {
  const { content } = JSON.parse(event.data);
  if ('thinking' in content) process.stdout.write(content.thinking);
  else if ('tool_choice' in content) console.log('\n[tool]', content.tool_choice.name);
  else if ('message' in content) console.log('\n', content.message.content);
  else if ('checkpoint' in content) console.log('\n[checkpoint]', content.checkpoint.handover);
  else if ('error' in content) console.error(content.error.kind, content.error.message);
};

// abort the in-flight response
function abort() {
  ws.send(JSON.stringify({ timestamp: new Date().toISOString(), user: 'alice',
    content: { command: 'abort' }, metadata: {} }));
}
```

### Upload a file and attach it

```javascript
const b64 = btoa(String.fromCharCode(...new Uint8Array(await file.arrayBuffer())));
const up = await fetch(`${base}/files/upload`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
  body: JSON.stringify({ file: b64, filename: file.name }),
});
const { data: { url } } = await up.json();

ws.send(JSON.stringify({
  timestamp: new Date().toISOString(),
  user: 'alice',
  content: { chat: 'What is in this document?' },
  metadata: {},
  attachments: [{ filename: file.name, content: { local: url } }],
}));
```

## Python

```python
import json, requests, websocket
from datetime import datetime, timezone

BASE = "http://localhost:9999/api/v1"

token = requests.post(f"{BASE}/auth/login",
                      json={"username": "alice", "password": "s3cret"}).json()["data"]["token"]
H = {"Authorization": f"Bearer {token}"}

# REST chat
r = requests.post(f"{BASE}/agents/assistant/chat", headers=H,
                  json={"channel_id": "py", "topic_id": "t1", "content": "Hello"})
print(r.json()["data"]["content"])

# WebSocket streaming
ws = websocket.create_connection(
    f"ws://localhost:9999/api/v1/agents/assistant/channel/py/topic/t1/chat?token={token}")
ws.send(json.dumps({
    "timestamp": datetime.now(timezone.utc).isoformat(),
    "user": "alice",
    "content": {"chat": "Give me three project ideas."},
    "metadata": {},
}))
while True:
    frame = json.loads(ws.recv())
    content = frame["content"]
    if "thinking" in content:
        print(content["thinking"], end="")
    elif "tool_choice" in content:
        print("\n[tool]", content["tool_choice"]["name"])
    elif "message" in content:
        print("\n", content["message"]["content"])
        break
    elif "error" in content or "abort" in content:
        print("\n[stopped]", content)
        break
```

## Memory and tasks

```sh
# Write a memory concept into bundle "projects"
curl -s localhost:9999/api/v1/agents/assistant/memory \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"title":"Vizier","bundle":"projects","path":"vizier","content":"Rust agent framework. See [[people/alice]].","tags":["rust"]}'

# Semantic search
curl -s "localhost:9999/api/v1/agents/assistant/memory/query?query=agent+framework&limit=5" \
  -H "authorization: Bearer $TOKEN" | jq '.data[].title'

# Export a bundle
curl -s -o projects.zip localhost:9999/api/v1/agents/assistant/memory/bundles/projects/export \
  -H "authorization: Bearer $TOKEN"

# Trigger a dream cycle
curl -s -X POST localhost:9999/api/v1/agents/assistant/dream/trigger -H "authorization: Bearer $TOKEN"
```
