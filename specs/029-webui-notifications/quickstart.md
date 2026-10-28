# Quickstart: WebUI Notifications

**Feature**: `029-webui-notifications`

This checks the feature against a running binary, mostly on the offline `dummyplug` provider (spec `008-dummyplug-provider`). The dummyplug protocol is `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`; agent tool calls go through `execute_python` (code mode). Steps marked **LIVE** need a real provider, Ollama or network and are not part of the offline gate; everything else is offline.

Run against a scratch workspace so the synthetic history and notifications do not pollute a real install:

```sh
rm -rf /tmp/vizier-notifications
cargo run -- run --workspace /tmp/vizier-notifications --port 9999 2>&1 | tee /tmp/vizier-notifications.log
```

On first run, complete onboarding in the browser (create the admin), or create it directly:

```sh
curl -s localhost:9999/api/v1/auth/setup \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"admin"}'
```

In another shell:

```sh
TOKEN=$(curl -s localhost:9999/api/v1/auth/login \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"admin"}' | jq -r .data.token)
API=localhost:9999/api/v1
auth=(-H "authorization: bearer $TOKEN" -H 'content-type: application/json')
```

Create a dummyplug agent `dp-a` (the WebUI's New agent flow works too; the offline e2e fixture uses `POST /api/v1/agents` with `provider: "dummyplug"`, `model: "dummyplug"`):

```sh
curl -s "${auth[@]}" "$API/agents" -d '{
  "agent_id":"dp-a","name":"Dp A","description":"notifications quickstart",
  "provider":"dummyplug","model":"dummyplug"
}' | jq .data.agent_id
```

## 1. Integration notification, counts and filters

Add an MCP integration (the command does not need to work; saving is the event):

```sh
curl -s "${auth[@]}" -X POST "$API/agents/dp-a/integrations" -d '{
  "kind":"mcp","name":"notify-mcp",
  "settings":{"transport":"local","command":"cat"}
}' | jq '.data.integration.name'
```

Expect `notify-mcp` and a `201`. Read the inbox:

```sh
curl -s "${auth[@]}" "$API/notifications" | jq '.data'
```

Expect one `category: "integration"`, `tone: "default"`, text `admin added notify-mcp.`, `href: "/dp-a/integrations/mcp"`, `read: false`, and counts with `unread` one higher than the rows that were there before the write. `admin` is the signed-in username; the actor name is resolved at creation, not read time.

A brand-new workspace can also show one `agent_setup_ready` (`Dp A is ready.`) if the agent's build downloaded the default local embedding model — that is the intended ready outcome, not a duplicate. Filter by category when the count matters:

```sh
curl -s "${auth[@]}" "$API/notifications" | jq '.data.items[] | select(.category=="integration")'
```

Check the filters and the badge endpoint:

```sh
curl -s "${auth[@]}" "$API/notifications/count" | jq .data
curl -s "${auth[@]}" "$API/notifications?filter=unread" | jq '.data.items | length'
curl -s "${auth[@]}" "$API/notifications?filter=read" | jq '.data.items | length'
```

Expect `{all:1, unread:1, read:0}`, then `1`, then `0`. An unknown filter is `400`.

## 2. Read state, mark-all and persistence

Take the id from §1 and mark it read:

```sh
ID=$(curl -s "${auth[@]}" "$API/notifications" | jq -r '.data.items[0].id')
curl -s "${auth[@]}" -X PUT "$API/notifications/$ID/read" -d '{"read":true}' | jq .data
```

Expect `{all:1, unread:0, read:1}`. The `read` filter now has the row; `unread` is empty. Repeat the same `PUT`; counts are unchanged (idempotent). Mark it unread:

```sh
curl -s "${auth[@]}" -X PUT "$API/notifications/$ID/read" -d '{"read":false}' | jq .data
```

Then `POST /notifications/read-all`:

```sh
curl -s "${auth[@]}" -X POST "$API/notifications/read-all" | jq .data
```

Expect `unread: 0`. Add a second integration (`notify-mcp-2`), check unread is 1, mark all read, then stop and restart the process (Ctrl-C, then rerun the same `cargo run -- run --workspace ...` command) and sign in again: the read state must survive. A missing/invisible id returns `404`:

```sh
curl -s -o /dev/null -w '%{http_code}\n' "${auth[@]}" -X PUT "$API/notifications/999999/read" -d '{"read":true}'
```

## 3. Task run notification (dummyplug)

Create a one-time task that fires in about a minute:

```sh
WHEN=$(date -u -d '+90 seconds' +%Y-%m-%dT%H:%M:%SZ)   # macOS: date -u -v+90S +%Y-%m-%dT%H:%M:%SZ
curl -s "${auth[@]}" -X POST "$API/agents/dp-a/tasks" -d "{
  \"slug\":\"notify-task\",\"title\":\"Notify task\",\"instruction\":\"Say hello\",
  \"schedule\":{\"type\":\"OneTime\",\"datetime\":\"$WHEN\"},\"is_active\":true
}" | jq '.data.slug'
```

Wait for the scheduler tick (it checks every minute) and for the dummyplug turn to answer. Poll until the task notification appears:

```sh
for i in $(seq 1 36); do
  curl -s "${auth[@]}" "$API/notifications?filter=all" | jq -r '.data.items[] | select(.category=="task_run") | .text' | grep -q . && break
  sleep 5
done
curl -s "${auth[@]}" "$API/notifications?filter=all" | jq '.data.items[] | select(.category=="task_run")'
```

Expect `Task “Notify task” answered.` (tone `default`) and the task-run href `.../tasks?task=notify-task&run=...`. A run that ends in `Abort`/`Empty`/no message becomes `ended without an answer.` and tone `failed`; a restart-interrupted run becomes `was interrupted.`. The title is captured at fire time; deleting the task later leaves the notification readable.

## 3b. WebUI chat reply (the sender only)

A WebUI conversation's reply is a notification for the person whose conversation it is, and for nobody else. In the browser: open an agent's chat, send a message, switch to another page, and wait for the reply. Exactly one `chat_reply` appears in that person's inbox, with text `Replied in your conversation.` and the conversation's href; it is not on the page itself (it is in the bell).

On the API, the same turn can be driven over the socket with the dummyplug protocol:

```sh
# as ana: send a chat turn on a topic of your own
# (the WebUI uses channel "vizier-webui"; any id works in the URL)
node - <<'JS'
const login = await fetch('http://localhost:9999/api/v1/auth/login', { method: 'POST', headers: {'content-type':'application/json'}, body: JSON.stringify({username:'admin', password:'admin'}) }).then(r => r.json())
const ws = new WebSocket(`ws://localhost:9999/api/v1/agents/dp-a/channel/vizier-webui/topic/notify-chat/chat?token=${login.data.token}`)
await new Promise((res) => (ws.onopen = res))
ws.send(JSON.stringify({ timestamp: new Date().toISOString(), user: 'admin', content: { chat: 'hello' }, metadata: {} }))
await new Promise((res) => setTimeout(res, 6000))
ws.close()
JS
curl -s "${auth[@]}" "$API/notifications?filter=all" | jq '.data.items[] | select(.category=="chat_reply")'
```

Expect `Replied in your conversation.` and `href: "/dp-a/chat/notify-chat"`. Sign in as a second person who can see `dp-a` and check their inbox: `chat_reply` is absent (list, counts and a per-id `PUT` all behave as not found). A turn that ends aborted/empty, a Discord/Telegram turn, and a background report add no `chat_reply`.

## 4. Background job notification (dummyplug)

Ask the agent for the tool schema and start a background batch from a script:

```json
{"tool":"describe_tool_function","arguments":{"name":"background_subagents"}}
```

Then send:

```json
{"tool":"execute_python","arguments":{"code":"print(background_subagents(tasks=[{'prompt': 'Say hello from a background piece'}], timeout_secs=30))"}}
```

Expect `Started background batch …`. When the piece answers and the report is delivered, a `background_job` notification appears:

```sh
curl -s "${auth[@]}" "$API/notifications?filter=all" | jq '.data.items[] | select(.category=="background_job")'
```

Expect `Background batch reported back.` and href `/{agent}/chat`. A cancelled job reads `was cancelled (reason).`; a job lost to a restart reads `was lost when vizier restarted.`; an undelivered report reads `finished, but its report could not be delivered.` A job still `running` or `reporting` has no notification.

## 5. Agent start failure (offline, custom provider)

This uses a real persisted agent whose provider is removed, not a dummyplug turn.

1. In Settings › Providers, add a custom OpenAI-compatible provider (any base URL) named `notify-custom`, then create an agent `dp-fail` using it. Let it start successfully.
2. Delete `notify-custom` in Settings › Providers. The agents using it restart; the build cannot resolve the provider.
3. Poll the inbox for a `agent_start_failure`:

```sh
for i in $(seq 1 36); do
  curl -s "${auth[@]}" "$API/notifications" | jq -r '.data.items[] | select(.category=="agent_start_failure") | .text' | grep -q . && break
  sleep 2
done
curl -s "${auth[@]}" "$API/notifications" | jq '.data.items[] | select(.category=="agent_start_failure")'
```

Expect one `Failed to start: … Retrying in …s.` with tone `failed` and `href: "/dp-fail/config"`. Wait through at least one retry; the count of this notification must stay **1** for the same failure episode. Open `dp-fail`'s chat: the failed banner (Retry now / Open config) must still be there; the detailed starting card must not be.

Recovery-ready is covered by the supervisor test: a successful build after a failed attempt **within the same supervisor retry loop** adds one `agent_setup_ready` even if the retry reused cached artifacts. A manual provider restore restarts the supervisor, so it is a new start rather than that retry; delete `dp-fail` when done.

Clean up by deleting `dp-fail` or recreating the provider.

## 6. Live setup, ready and the download rule — **LIVE**

Use a scratch agent on Ollama, or restart vizier after removing the model from Ollama so the boot build downloads it asynchronously.

1. With the model absent (`ollama rm <model>`), restart vizier. Open Notifications on another screen.
2. While the build runs, the header's bell shows the **activity dot**, not an unread count for the setup. The panel's Setup section shows `Starting <name>…`, the current step, percent/bytes when known, an indeterminate bar when the size is unknown, the ETA when measurable, and one line per download; it updates about once a second. The card is not in the list and has no read action.
3. When the download finishes, one unread `agent_setup_ready` appears: `<name> is ready.` (tone `success`), the activity indicator clears, and the Setup section disappears.
4. Restart vizier again with the model cached. A quick restart shows live activity only and adds **no** ready notification.
5. Break the pull (for example stop Ollama after the build starts, or use an unreachable base URL) and restart: the start-failure notification is the one from §5; the setup board does not add a second failure. The chat's compact line explains the queue; the failure banner remains.

The offline Rust tests cover the download epoch, the ready flag and one-failure-per-episode; the live run verifies the real progress UI and the real Ollama download.

## 7. Channel-reply projection (offline fixture)

The current hour must not appear; a closed hour must become exactly one notification per conversation. With `dp-a` created:

```sh
DB=/tmp/vizier-notifications/.runtime/vizier.db
HOUR_MS=3600000
NOW=$(date +%s%3N)
PREV_HOUR=$(( NOW / HOUR_MS - 1 ))
PREV=$(( PREV_HOUR * HOUR_MS + 60000 ))   # one minute into the previous hour
# The app may already have projected this hour; rewind the watermark so this fixture
# is treated as a newly closed batch (the projection is idempotent).
sqlite3 "$DB" "INSERT OR REPLACE INTO state (key,value) VALUES ('notifications.replies_through','$((PREV_HOUR-1))');"
sqlite3 "$DB" <<SQL
INSERT INTO session_history (uid, agent_id, channel, topic, timestamp, content_type, data, seq)
VALUES ('fixture-r1','dp-a','discord__fixture',NULL,$PREV,'Response','{}',1),
       ('fixture-r2','dp-a','discord__fixture',NULL,$((PREV+5000)),'Response','{}',2);
SQL
curl -s "${auth[@]}" "$API/notifications" | jq '.data.items[] | select(.category=="channel_replies")'
```

Expect one `Replied 2 times in a Discord conversation.` with href `/dp-a/chat?channel=discord__fixture`, created at the later reply. Inserting a third row in the **current** hour and calling again must not add or change the batch. Restarting and calling again must not duplicate it (the unique `reply:{agent}:{channel}:{hour}` key and the watermark).

The first activation watermark starts at the last closed hour, so no historical replies are imported; a 30-day catch-up is capped by the retention window.

## 8. Dream notification — **LIVE**

Dummyplug produces no journal entries, so this is a live check:

1. Use a real provider; enable the dream cycle for an agent with at least one user conversation.
2. Trigger a dream (`POST`/the WebUI Dream button) and let it finish.
3. Expect one `dream_cycle` notification: `Dreamed: 1 journal entry.` / `Dreamed: {n} journal entries.` with the dream-cycle href. A cycle that wrote no entries produces none.

The offline test covers the count/wording path; this verifies the real consolidation writes entries.

## 9. Visibility, retention and agent deletion

1. As a person who may see only `dp-a`, `GET /notifications` contains only `dp-a` rows; `GET /notifications/count` does not count another agent's rows. Asking to mark another agent's id read returns `404`.
2. Revoke that person's access to `dp-a`; reload: the rows and the unread count are gone from their view, while the notification rows remain for people who can see it.
3. Insert an old notification directly to verify the window:

```sh
OLD=$(( $(date +%s%3N) - 31*24*3600*1000 ))
sqlite3 "$DB" "INSERT INTO notification (event_key,agent_id,category,tone,summary,href,created_at)
VALUES ('fixture-old','dp-a','task_run','default','old','/dp-a/tasks','$OLD');"
curl -s "${auth[@]}" "$API/notifications" | jq '[.data.items[] | select(.text=="old")] | length'
```

Expect `0`, and the count endpoint must not include it. Restart (or the daily purge) and check `select count(*) from notification where event_key='fixture-old';` is `0`.

4. Delete `dp-a`; check:

```sh
sqlite3 "$DB" "select count(*) from notification where agent_id='dp-a';
                select count(*) from notification_read r join notification n on n.id=r.notification_id where n.agent_id='dp-a';"
```

Expect `0` and `0` (the read rows cascade). Existing notifications for other agents are untouched.

## 10. Home, header bell, phone and accessibility

With at least one unread notification:

1. On a desktop, open any screen: the page header's right end shows the bell, with the unread count as a badge and a separate dot while an agent sets up. The bell is present on every page (the shared `PageFrame`, Settings, Home and the chat header).
2. Open it: a right-hand 440px panel over the current screen. Unread / Read / All show counts and it opens on `Unread`; mark one read, mark another unread, then `Mark all read`; the badge clears and the list updates. Each row is a card with a large agent sigil, the agent and time on one line, the summary under it, and the mark-read check inside the card at the right. Open a row: the destination opens and the row is read when you come back. Escape closes and focus returns to the bell.
3. Leave the panel closed on another screen and let an event happen (a task run, or a chat reply): a toast appears top-right, under the page header, within about 15 seconds, naming the agent and the event. Stay in the conversation instead and send a message: its reply is read on arrival, with no toast and no badge change; a reply for a different conversation still toasts. Clicking it opens the destination and marks it read; it dismisses itself after about 6 seconds (8 for a failure). Several arrivals at once collapse to one `N new notifications` toast that opens the panel.
4. Home no longer contains `Since you were away` or a reserved right column; the greeting, activity grid and court use the width. Mobile Home is unchanged.
5. At 390px, the top bar shows the same bell: it opens a full-screen sheet with the same content. The setup card is full width, the filters fit/wrap, the mark action is always visible (no hover), and closing returns to the previous screen. The panel has no horizontal scroll and clears the bottom safe area.
6. Each row's eye (closed = unread, open = read) sits left of the time; clicking the card outside the panel's surface (the ground behind it) closes the panel, as does Escape.
7. With a screen reader, the bell announces the unread count and busy state; the dialog is named `Notifications`; each row announces the agent, unread state, text and time; the action has a label; failure is not conveyed by colour alone.
8. Open the chat: the conversation details side panel starts collapsed (the layers button opens it); a starting agent shows only the compact `Starting … · View in Notifications` line and the queue note, with the detailed card in the notifications panel. A failed agent still has the failed banner with Retry now / Open config.

Playwright covers the same paths: `cd webui && npm run e2e -- notifications.spec.ts`. The phone project asserts the full-screen dialog and no horizontal overflow.

## 11. Final checks

```sh
cargo fmt --check
cargo clippy --all-targets
cargo test
cd webui && npm test && npm run typecheck && npm run e2e
```

All must pass. Confirm the old feed is gone (`grep -r "overview/feed\|Since you were away\|AwayFeed" src webui docs` → no hits) and that a notification read/write needs no network. Delete `/tmp/vizier-notifications` when done.

## Recorded run — 2026-10-25 (offline parts)

Run on a `--workspace /tmp/vizier-notify-ws --port 19999` scratch install, dummyplug agent `scribe`, signed in as `ana`.

| Step | Result |
|---|---|
| §1 integration | `201`; inbox row `{category: "integration", tone: "default", text: "ana added scribe-fs.", href: "/scribe/integrations/mcp", read: false}`; `GET /notifications/count` → `{all: 2, unread: 2, read: 0}` (the other row was `agent_setup_ready` from the embedding download during the create build, as noted above) |
| §1 errors | unknown filter → `400 unknown filter`; malformed cursor → `400 malformed cursor`; missing `read` → `400 a read boolean is required`; bad token → `401`; unknown id → `404 notification not found` |
| §1 paging | `limit=1` returned the newest row plus a `next`; following it returned the older row with no `next` |
| §2 read state | `PUT {read:true}` → `{all: 2, unread: 1, read: 1}`; `filter=unread` listed only the unread row, `filter=read` only the read one; `POST /read-all` → `{all: 2, unread: 0, read: 2}` |
| §2 persistence | killed the process, restarted it: the three rows kept their ids and `read` flags (`3` unread, `2`/`1` read), and the startup sweep added nothing |
| §3 task run | one-time `morning-digest` fired on the next tick: `{category: "task_run", tone: "default", text: "Task “Morning digest” answered.", href: "/scribe/tasks?task=morning-digest&run=1", read: false}` |
| §3b chat reply | a WebUI turn on topic `topic-abc` produced `{category: "chat_reply", text: "Replied in your conversation.", href: "/scribe/chat/topic-abc"}` for `ana` (`counts {all: 1, unread: 1}`); `bob`, a superadmin who can also see `scribe`, saw `{all: 0, unread: 0}` |
| §7 replies | rewound `notifications.replies_through` and inserted two Discord + one Telegram response in the previous hour: one `Replied 2 times in a Discord conversation.` and one `Replied once in a Telegram conversation.` appeared, with the conversation hrefs; a second call returned the same counts (idempotent) |
| §4 background job | sent the quickstart's `background_subagents` call through the WebSocket dummyplug protocol; after the piece answered and the report was delivered, exactly one `{category: "background_job", text: "Background batch reported back.", href: "/scribe/chat"}` appeared (nothing while it was `running`/`reporting`) |
| §5 start failure | deleted the custom provider `notify-custom` under agent `dp-fail` (`DELETE …?confirm=true`): one `{category: "agent_start_failure", tone: "failed", text: "Failed to start: agent failed to start: provider \"notify-custom\" is not available; pick another provider. Retrying in 2s.", href: "/dp-fail/config"}` despite the supervisor reaching `failures: 4` |
| §5 recovery-ready | re-created the provider: the next retry came up and added exactly one `Dp Fail is ready.` (`success`); the failure count stayed 1 |
| §6 quick restart | `POST /agents/dp-fail/restart` (no download, no failure episode): the ready count stayed 1 — a quick restart adds no notification |
| §9(4) delete | a second agent `atlas` got `ana added atlas-fs.`; deleting it removed that row while `scribe`'s rows and read states stayed |

Not run here, still to be checked on a machine with the live dependencies:

- §6 Ollama download → ready (the download branch; the quick-restart branch was checked above) needs a real model pull (LIVE).
- §8 dream: LIVE, a real provider writes journal entries.
- §10 browser checks and Playwright: the design/webui Playwright browser cannot start in this environment (`libnspr4.so` missing), so `npm test` was run but `npm run e2e` was not.

UI refinements after the first pass (arrival toast, card rows with a larger sigil and no unread dot) are covered by the webui store tests for arrival detection and by typecheck/build; they were not visually verified here (no browser).

Automated gates that did run:

```text
cargo test              722 passed, 0 failed, 7 ignored
cargo clippy --all-targets   (see the run in T082)
cd webui && npm test    483 passed
cd webui && npm run typecheck   clean
cd design && npm run typecheck  clean
cd design && npm run build-storybook  built
```
