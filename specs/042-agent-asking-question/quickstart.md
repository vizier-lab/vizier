# Quickstart: Asking the Person — questions and sensitive input

**Feature**: `042-agent-asking-question`

End-to-end checks against a running binary with an agent on the offline `dummyplug` provider
(protocol: `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`; standalone server:
`specs/030-standalone-dummyplug-server/`). Steps marked **LIVE** need a real provider, a Discord
bot, a Telegram bot or a browser, and are not part of the offline gate.

> **Scripts return their last expression.** The sandbox returns the value of the script's final
> *expression*, so end a script with the value on a line of its own (`out`, not `out = …`). A turn
> that ends on `ask_question` returns no reply text; read the script's `stdout` from the stored
> tool result (`HIST <topic> | jq -r '.data[].content.ToolResult.content? | select(.!=null)'`).
> Use a scratch directory instead of `/tmp` if your machine keeps it small.

## Setup

```sh
just dummyplug    # terminal 1: the dummyplug server on 127.0.0.1:9998
rm -rf /tmp/vizier-ask
VIZIER_JWT_SECRET=any-long-random-string cargo run -- run --workspace /tmp/vizier-ask --port 9999 2>&1 | tee /tmp/vizier-ask.log   # terminal 2
```

```sh
curl -s localhost:9999/api/v1/auth/setup -H 'content-type: application/json' \
  -d '{"username":"admin","password":"admin"}'
TOKEN=$(curl -s localhost:9999/api/v1/auth/login -H 'content-type: application/json' \
  -d '{"username":"admin","password":"admin"}' | jq -r .data.token)
API=localhost:9999/api/v1
auth=(-H "authorization: bearer $TOKEN" -H 'content-type: application/json')
curl -s "${auth[@]}" "$API/agents" -d '{
  "agent_id":"dp-ask","name":"Dp Ask","description":"questions quickstart",
  "provider":"dummyplug","model":"dummyplug"
}' | jq .data.agent_id

# ASK <topic> <script.py> <intent> — dummyplug runs the script in a real execute_python call.
ASK() {
  BODY=$(jq -n --rawfile code "$2" --arg intent "$3" \
    '{tool:"execute_python",arguments:{code:$code,intent:$intent}}')
  curl -s "${auth[@]}" "$API/agents/dp-ask/chat" \
    -d "$(jq -n --arg t "$1" --arg c "$BODY" '{channel_id:"webui",topic_id:$t,content:$c}')" \
    | jq -r '.data.content'
}
HIST() { curl -s "${auth[@]}" "$API/agents/dp-ask/channel/webui/topic/$1/history" | jq '.data'; }
```

## 1. Both tools are offered in an interactive turn (US6-1, US7-1, FR-007, FR-009)

```sh
printf 'sorted({t["category"] for t in list_tools()})\n' > /tmp/cats.py
ASK ask1 /tmp/cats.py 'list the tool categories this turn is offered'
```

Expect `questions` and `secrets` in `result`. Then the description the agent actually reads:

```sh
printf 'print(describe_tool_function(name="ask_question")["description"])\n' > /tmp/d.py
ASK ask1 /tmp/d.py 'read the question tool description'
```

Expect one `ENDS_TURN_NOTICE` occurrence (`grep -c 'Calling this ends your turn'` on the printed
text → `1`), the four question types, the 10-question limit, the answer shape, and the pointer
to `ask_sensitive_input` for credentials (FR-009, US6-2, FR-066).

## 2. A valid question set is presented and ends the turn (US1-1, FR-006, SC-003)

```sh
cat > /tmp/q1.py <<'PY'
ask_question(
    title="Pick one",
    questions=[{
        "id": "db", "prompt": "Database", "type": "choice",
        "options": [{"value": "postgres", "label": "PostgreSQL"},
                    {"value": "sqlite", "label": "SQLite"}],
    }],
)
PY
ASK ask2 /tmp/q1.py 'ask which database to use'
```

Expect `result` = `{"status": "pending", "question_set_id": "…"}`, the turn to end with
`stats.ended_by_tool == "ask_question"`, and **no** further model round: the log shows one
completion for the turn. `HIST ask2` then holds a `QuestionSet` entry in state `pending`.

## 3. Answering it delivers a structured answer turn (US1-2, FR-013, FR-015, SC-004)

The answer is an ordinary **request over the topic WebSocket** (`contracts/questions-wire.md`
§2); the REST `chat` route takes string content only. Any WebSocket client works. This one
uses Node's built-in `WebSocket`:

```sh
cat > /tmp/ws.mjs <<'JS'
// node ws.mjs <topic> <seconds> '<json request>' …  — prints each frame received
const [topic, secs, ...sends] = process.argv.slice(2)
const ws = new WebSocket(`ws://localhost:9999/api/v1/agents/dp-ask/channel/webui/topic/${topic}/chat?token=${process.env.TOKEN}`)
ws.onopen = () => sends.forEach((s) => ws.send(s))
ws.onmessage = (m) => console.log(m.data)
setTimeout(() => { ws.close(); process.exit(0) }, Number(secs) * 1000)
JS
SET=$(HIST ask2 | jq -r '[.data[] | .content.QuestionSet? | select(. != null and .state=="pending") | .set_id] | last')
MSG=$(jq -nc --arg s "$SET" '{timestamp:"2026-11-01T10:31:00Z",user:"admin",metadata:{},
  content:{question_answer:{set_id:$s,declined:false,
  answers:[{question_id:"db",state:"answered",value:"sqlite"}]}}}')
TOKEN=$TOKEN node /tmp/ws.mjs ask2 6 "$MSG"
```

Expect a `question_set` frame with `state: "answered"`, then the agent's turn (`thinking_start`,
then a `message`). `HIST ask2` shows that set as `answered` with `answers[0].value == "sqlite"`,
plus a `Request` entry whose content is the `question_answer`, and the `question_answer`
reached the agent as JSON, not as an opaque string (FR-062).

## 4. An invalid set is refused and the turn continues (US1-8, FR-005)

```sh
cat > /tmp/q2.py <<'PY'
try:
    out = ask_question(questions=[
        {"id": "a", "prompt": "A", "type": "choice", "options": []},
        {"id": "a", "prompt": "", "type": "text"},
    ])
except Exception as e:
    out = str(e)
out
PY
ASK ask3 /tmp/q2.py 'ask a deliberately broken question set'
```

Expect `result` to name **both** problems (e.g. `no_options` and `duplicate_question_id`),
`ended_by_tool` absent, a closing completion, and no `QuestionSet` entry in `HIST ask3`.

## 5. A second set in the same turn is refused (FR-008)

```sh
cat > /tmp/q3.py <<'PY'
one = {"questions": [{"id": "x", "prompt": "X", "type": "text"}]}
print(ask_question(**one))
try:
    print(ask_question(**one))
except Exception as e:
    print("second refused:", e)
PY
ASK ask4 /tmp/q3.py 'ask one set, then try to ask another'
```

Expect the first pending, the second refused with a message naming the one-set-per-turn rule,
and exactly one `QuestionSet` entry in `HIST ask4`.

## 6. Availability: absent where nobody can answer (US6-3, US7-3, SC-017)

```sh
curl -s "${auth[@]}" "$API/agents/dp-ask/tasks" -d '{
  "slug":"askt","cron":"0 0 1 1 *","prompt":"tools",
  "channel_id":"task"}' | jq .data.slug
```

Trigger the run, then read the tool list the run saw (`list_task_runs`, then
`get_task_run_detail`): neither `ask_question` nor `ask_sensitive_input` appears, and the
`questions` category line is absent while `secrets` still lists `set_secret`/`get_secret`. A
script naming one fails with "does not exist" and the turn continues. The same holds for a
`/btw` side chat:

```sh
curl -s "${auth[@]}" "$API/agents/dp-ask/btw" -d '{"channel_id":"webui",
  "topic_id":"ask1","question":"tools"}' | jq -r .data.answer | grep -c ask_
```

Expect `0`.

## 7. A pending set survives a restart (US3-9, FR-016, FR-025, SC-011)

Ask a set on topic `ask6` (repeat §2 against `ask6`), then restart the binary **without**
answering it. After the restart `HIST ask6` still shows it `pending` and is still answerable by
§3's request. On Discord and Telegram a control posted before the restart still resolves, because
it carries only the short id resolved from `question_set`.

## 8. Answer validation (US2-4, US9-1, US9-2, FR-012, FR-017)

Submit a value not among the options, then an over-limit text: each is refused with
`invalid_answer` naming the question, the set stays `pending`, and no turn starts. Submit the
same valid answer twice: the first wins, the second gets `question_not_open`.

## 9. **LIVE** — a sensitive value reaches only the script (US4, US5, SC-014, SC-015, SC-023)

```sh
cat > /tmp/s1.py <<'PY'
v = ask_sensitive_input(
    purpose="A GitHub token with repo scope, to file the issue you asked for.",
    label="GitHub token",
)
print(v["value"])
v["status"]
PY
ASK ask5 /tmp/s1.py 'ask for a token and use it'
```

With a WebUI socket open on `ask5`, the `sensitive_ask` frame arrives; type
`quickstart-token-abc123` into the dialog and confirm. Then:

```sh
grep -c 'quickstart-token-abc123' /tmp/vizier-ask.log        # 0
# (the database is /tmp/vizier-ask/.runtime/vizier.db)
HIST ask5 | grep -c quickstart-token-abc123                  # 0
```

The same check, scripted so it can be re-run (SC-022). The value must be absent from **every
text column of every table**, not only `session_history`:

```sh
VALUE=quickstart-token-abc123
DB=/tmp/vizier-ask/.runtime/vizier.db     # the live database is under the workspace's .runtime/
echo "log:     $(grep -c "$VALUE" /tmp/vizier-ask.log)"
echo "history: $(HIST ask5 | grep -c "$VALUE")"
total=0
for t in $(sqlite3 $DB "select name from sqlite_master where type='table' and name not like 'sqlite_%'"); do
  cols=$(sqlite3 $DB "select group_concat('coalesce(cast(\"'||name||'\" as text),\"\")', '||') from pragma_table_info('$t')")
  n=$(sqlite3 $DB "select count(*) from \"$t\" where ($cols) like '%$VALUE%'" 2>/dev/null || echo 0)
  total=$((total + ${n:-0}))
done
echo "tables:  $total"          # all three are 0
```

(`sqlite3` reports `no such module: vec0` for the vector-index table; it holds embeddings only.)

and `HIST ask5` shows `[redacted]` where the script printed the value, a `SensitiveAsk` entry in
state `provided`, and no value anywhere. The turn is still running when the ask closes
(`stats.ended_by_tool` absent) — SC-023.

**LIVE (browser)**: the field masks by default, offers a reveal, does not autofill, does not
offer itself to the password manager, and does not echo the value after submit. Reject, Escape,
an outside press and the close control each close the dialog; the script gets
`{"status": "declined"}` and the turn finishes normally (FR-036, FR-050, SC-024). A second tab
shows the same dialog and the first submission closes both (FR-048).

**LIVE**: supply a 4-character value — the dialog states before submission that a value this
short cannot be scrubbed, and the script's result carries `protected: false` (FR-043, SC-019).

## 10. **LIVE** — the channels (US3, SC-005, SC-013)

With a Discord bot and a Telegram bot attached to the same agent, ask the same two-question set
(`db` choose-one, `features` choose-several) in each conversation:

- Discord: buttons for `db`; a select with `min`/`max` plus a submit for `features`; the message
  is edited into a record of the answers and its components are gone. 50 presses all acknowledge
  within 3 s, none shows "interaction failed" (SC-013).
- Telegram: one inline keyboard per question, all carrying the same short id; pressing toggles
  for `features`, then submit; each message is edited to its answer with its keyboard removed.
- A press on a closed or already-answered control shows a short notice and starts no turn.
- A set with more than 25 options falls back to the text form of
  `contracts/channel-controls.md` §4, and replying to that message answers the whole set in one
  submission.
- Neither tool appears in a Discord or Telegram conversation's `tools` listing except
  `ask_question`; `ask_sensitive_input` is absent there (FR-046, SC-017).

## 11. Cleanup

```sh
curl -s "${auth[@]}" -X DELETE "$API/agents/dp-ask/channel/webui/topic/ask6" | jq .
sqlite3 /tmp/vizier-ask/.runtime/vizier.db "select count(*) from question_set where topic='ask6'"   # 0
sqlite3 /tmp/vizier-ask/.runtime/vizier.db "select count(*) from sensitive_ask where topic='ask5'"  # 0
```

Deleting the conversation removes its sets and asks with it (FR/US9 #9).
