# Quickstart: /btw Ephemeral Side-Chat

**Feature**: `033-btw-side-chat`

End-to-end checks against a running binary with an agent on the offline `dummyplug` provider (protocol: `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`; standalone server: `specs/030-standalone-dummyplug-server/`). Steps marked **LIVE** need a real provider, a Discord bot or a Telegram bot and are not part of the offline gate.

## Setup

```sh
just dummyplug    # terminal 1: the dummyplug server on 127.0.0.1:9998
rm -rf /tmp/vizier-btw
cargo run -- run --workspace /tmp/vizier-btw --port 9999 2>&1 | tee /tmp/vizier-btw.log   # terminal 2
```

```sh
curl -s localhost:9999/api/v1/auth/setup -H 'content-type: application/json' \
  -d '{"username":"admin","password":"admin"}'
TOKEN=$(curl -s localhost:9999/api/v1/auth/login -H 'content-type: application/json' \
  -d '{"username":"admin","password":"admin"}' | jq -r .data.token)
API=localhost:9999/api/v1
auth=(-H "authorization: bearer $TOKEN" -H 'content-type: application/json')
curl -s "${auth[@]}" "$API/agents" -d '{
  "agent_id":"dp-btw","name":"Dp Btw","description":"btw quickstart",
  "provider":"dummyplug","model":"dummyplug"
}' | jq .data.agent_id
```

Give the host conversation one ordinary exchange (this one **is** saved):

```sh
curl -s "${auth[@]}" "$API/agents/dp-btw/chat" \
  -d '{"channel_id":"webui","topic_id":"host1","content":"hello there"}' | jq -r .data.content
HIST() { curl -s "${auth[@]}" "$API/agents/dp-btw/channel/webui/topic/$1/history" | jq '.data | length'; }
TITLE() { curl -s "${auth[@]}" "$API/agents/dp-btw/channel/webui/topics" | jq -r ".data[] | select(.topic_id==\"$1\") | .title"; }
BEFORE=$(HIST host1); echo "history entries: $BEFORE"
```

Wait a few seconds for the topic's generated title, then note it: `TITLE host1`.

## 1. A side chat answers and leaves no trace (US1, FR-006, SC-002)

```sh
curl -s "${auth[@]}" "$API/agents/dp-btw/btw" \
  -d '{"channel_id":"webui","topic_id":"host1","question":"what did I just say?"}' | jq .
```

Expect `200` and `data.answer` non-empty lorem ipsum (dummyplug §5), `data.stats.model == "dummyplug"`. Then:

```sh
[ "$(HIST host1)" = "$BEFORE" ] && echo "history unchanged"
TITLE host1        # same title as before, and is_thinking stayed false
```

Expect "history unchanged" and the same title. In `/tmp/vizier-btw/` no `session_turn` row is left (`sqlite3 /tmp/vizier-btw/vizier.db 'select count(*) from session_turn'` → `0`).

## 2. It sees the conversation, its identity and the side chat framing (US1-1, FR-002, FR-003)

```sh
curl -s "${auth[@]}" "$API/agents/dp-btw/btw" \
  -d '{"channel_id":"webui","topic_id":"host1","question":"context"}' | jq -r .data.answer
```

Expect the context block (dummyplug §6): it starts `# Context`, has `## Time`, and contains the `## Side Chat` section from `contracts/agent-side-chat.md` ("This is an off-the-record side chat…"). It has no `## Related Memories` section.

To confirm the host history and system prompts reach the model, run vizier with `RUST_LOG=vizier=debug` or put a recording proxy at `DUMMYPLUG_URL` (spec 030 test-controls) and check the request for the side chat: its `messages` start with the same system messages as the `hello there` request, then that exchange's user and assistant messages, then the side-chat question; `tool_choice` is `"none"`.

## 3. Follow-ups carry the exchange (US1-2, FR-005)

```sh
curl -s "${auth[@]}" "$API/agents/dp-btw/btw" -d '{
  "channel_id":"webui","topic_id":"host1","question":"context",
  "turns":[{"question":"first aside","answer":"first answer"}]
}' | jq -r .data.answer | head -3
```

At the recording proxy, the request's last three messages are `user: first aside`, `assistant: first answer`, then the side-chat question. History is still `$BEFORE`.

The limit is 32,000 characters for the whole side chat (every earlier question and answer plus the new question), not a number of questions:

```sh
A=$(head -c 31998 /dev/zero | tr '\0' a)                      # 1 + 31,998 + 1 = 32,000
EX=$(jq -nc --arg a "$A" '[{question:"q", answer:$a}]')
curl -s -o /dev/null -w '%{http_code}\n' "${auth[@]}" "$API/agents/dp-btw/btw" \
  -d "{\"channel_id\":\"webui\",\"topic_id\":\"host1\",\"question\":\"x\",\"turns\":$EX}"   # 200
curl -s "${auth[@]}" "$API/agents/dp-btw/btw" \
  -d "{\"channel_id\":\"webui\",\"topic_id\":\"host1\",\"question\":\"xy\",\"turns\":$EX}" | jq .   # 422, side chat is full
EX=$(jq -nc '[range(20) | {question:"q", answer:"a"}]')
curl -s -o /dev/null -w '%{http_code}\n' "${auth[@]}" "$API/agents/dp-btw/btw" \
  -d "{\"channel_id\":\"webui\",\"topic_id\":\"host1\",\"question\":\"21st\",\"turns\":$EX}"   # 200: many short turns are fine
```

The 422 is refused before the agent is asked: no new `usage_event` row (§7).

## 4. No tools, and it says so (US1-4, FR-004, FR-013, SC-005)

```sh
curl -s "${auth[@]}" "$API/agents/dp-btw/btw" -d '{
  "channel_id":"webui","topic_id":"host1",
  "question":"{\"tool\":\"execute_python\",\"arguments\":{\"code\":\"print(1)\"}}"
}' | jq -r .data.answer
```

Dummyplug ignores `tool_choice: none` and answers with a tool call (§3); the side chat drops it. Expect exactly `I can't use tools in a side chat. Ask me in the conversation if you'd like me to do that.` History is still `$BEFORE`, and the log has no `execute_tool` line for this request.

## 5. A brand-new conversation is not created (edge case)

```sh
curl -s "${auth[@]}" "$API/agents/dp-btw/btw" \
  -d '{"channel_id":"webui","topic_id":"never-used","question":"hi"}' | jq -r '.data.answer | length > 0'
curl -s "${auth[@]}" "$API/agents/dp-btw/channel/webui/topics" | jq '[.data[] | select(.topic_id=="never-used")] | length'
```

Expect `true`, then `0`.

## 6. Rate limit (FR-010)

```sh
for i in 1 2 3 4 5 6; do
  curl -s -o /dev/null -w '%{http_code} ' "${auth[@]}" "$API/agents/dp-btw/btw" \
    -d '{"channel_id":"webui","topic_id":"host1","question":"hi"}'
done; echo
```

Earlier steps already spent some of the budget, so wait 60 s first. Expect `200 200 200 200 200 429`; the `429` carries `Retry-After` and costs nothing (no new `usage_event` row, nothing queued). After 60 s a request succeeds again.

## 7. Usage is still counted (FR-011, SC-006)

```sh
sqlite3 /tmp/vizier-btw/vizier.db \
  "select count(*) from usage_event where agent_id='dp-btw' and topic='host1'"
```

Expect one row per answered call in this topic: the `hello there` turn's rows plus one per `200` side chat above (dummyplug reports zero tokens; the rows still exist). `GET $API/agents/dp-btw/usage` includes them under the WebUI channel.

## 8. Queueing, abort and restart (FR-012)

- Ask a side chat while a slow turn runs in `host1` (put a delaying proxy at `DUMMYPLUG_URL`, spec 030): it answers after the turn ends.
- With a side chat held at the proxy, press the WebUI stop button on `host1` (it sends `abort` on the topic's socket): the side chat request returns `500` with the abort message.
- With a side chat held at the proxy, `PUT` the agent (any config change): the request returns `503` (`AgentUnavailable`); `select count(*) from session_turn` is `0`.
- With a side chat held, stop vizier (Ctrl-C) and start it again: the client sees a failed request; nothing is resent after startup (no `side chat` work in the log, history unchanged).

## 9. WebUI (US1, FR-007)

`cd webui && npm run dev`, open the `host1` conversation of `dp-btw` (the default local channel is `webui`; adjust `channel_id` above if yours differs).

1. Type `/b` — the slash menu lists `btw`.
2. Send `/btw what did I just say?` — the **Side chat** panel opens, shows the question and a lorem answer, the model `dummyplug` read-only, and "Not saved — closing this panel discards it." The main message list does not change, and nothing is added to the conversation list.
3. Ask `context` in the panel — the answer shows the `## Side Chat` section.
4. Close the panel, reload the page: no trace of the exchange anywhere in the conversation.
5. Send 6 side-chat questions quickly — the sixth shows "Slow down — try again in …s" inline.
6. Typecheck: `cd webui && npm run typecheck`.

## 10. Discord (US2) — **LIVE**

With a Discord channel integration on `dp-btw` and a second Discord account (B) in the same channel as the first (A):

1. A runs `/btw message:hello`. Everyone sees "A used /btw", then a lorem answer whose first line is the marker `🤫 side chat — visible to everyone, not remembered · reply to follow up`.
2. A replies `context` to that answer. The bot replies with the context block, containing `## Side Chat`.
3. B replies `hi` to the same answer. There's no answer, and the vizier log shows no request for B's reply.
4. A waits 5 minutes, then replies to an answer again. The bot replies "This side chat has ended — start a new one with /btw." with the marker. Replying to that notice gets the same notice.
5. A starts an aside, vizier is restarted, then A replies to its answer. Same "has ended" notice.
6. The messages are all still in the channel. The channel's conversation history (`GET …/channel/discord__…/topic/…/history` or the WebUI) holds none of the questions, answers or replies from steps 1–5 (SC-003). B's ordinary messages elsewhere in the channel are still recorded as `SilentRead`.
7. Over the limit: A runs `/btw` six times within a minute. The sixth gets an ephemeral "Slow down…" only A can see.

## 11. Telegram (US3) — **LIVE**

With a Telegram channel integration on `dp-btw`:

1. In a private chat: `/btw hello` — a reply with the "🤫 Side chat — not saved…" notice and a lorem answer.
2. Reply to the answer with `context` — the answer contains `## Side Chat`.
3. Wait 2 minutes 15 seconds — the `/btw` message, the follow-up and both answers are gone.
4. In a group where the bot is **not** an admin: `/btw hello` — the notice includes "if I'm allowed to delete them here"; after the timeout the bot's replies are gone and the person's `/btw` message remains.
5. Start an exchange, kill vizier before the timeout, restart it after the deadline: once the reader reconnects, the messages are deleted. While waiting, `select value from state where key like 'side_chat_pending__%'` shows only chat id, message ids and a time.
6. The chat's conversation history has no trace, and a group member's ordinary messages around the `/btw` are still recorded as `SilentRead` as before.
7. **Late reply (C1).** In a private chat, start an exchange and wait 2 minutes, then within the next ~10 s (before the sweep deletes it) reply `are you there?` to the answer. Expect "This side chat has ended — start a new one with /btw."; shortly after, the reply and that notice are deleted too. The conversation history has no `are you there?` entry.
8. **Reply after a restart (C1).** Start an exchange, stop vizier, start it again before the 2-minute deadline, and reply to the still-visible answer. Expect the same "has ended" notice, no new history entry, and everything deleted by the original deadline (or 2 minutes after the reply, if that deadline passed).
9. **Someone else's reply (G1).** In a group, a second member replies to the first member's side-chat answer. Expect no answer from the agent, no history entry for that reply, and (if the bot is an admin) the reply deleted with the exchange.

## 12. Automated checks

```sh
cargo clippy
cargo test side_chat          # validation, limits, SideChats sweep/lifetime, Telegram ledger merge
cargo test process::tests     # Btw is not durable; dispatch never reaches handle_request
cd webui && npm run typecheck && npx vitest run side
```
