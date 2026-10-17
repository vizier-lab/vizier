# Quickstart: Client Tools (dummyplug e2e)

**Feature**: `019-client-tools`

Every step runs against a real binary with an agent on the offline `dummyplug` provider, as the constitution's e2e gate requires. No step needs a live model. Steps marked **(browser)** use the WebUI. The others use the Node test client, `scripts/client-tools-e2e.mjs`, which runs all of them in sequence and exits non-zero on the first failed check.

## Setup

```sh
just install && just run          # dev.vizier.yaml, http on :9999
# In the WebUI: create agent "plug" with provider dummyplug. Note the JWT (Settings › API keys, or the login response).
export VIZIER_URL=ws://localhost:9999 VIZIER_TOKEN=<jwt> AGENT=plug
node scripts/client-tools-e2e.mjs   # runs §1–§8 below
```

The client connects to `/api/v1/agents/$AGENT/channel/e2e/topic/<fresh uuid>/chat`. Dummyplug runs a sandbox tool when the message is the JSON `{"tool": …, "arguments": …}` (dummyplug protocol §3).

## §1 Declaration is refused when invalid (FR-002)

Send `{"chat": "tools"}` with `client_tools: [{ "name": "Bad-Name", "description": "x", "input_schema": {"type":"object"} }]`.

**Expect**: `{"client_error": {"code": "invalid_client_tools", "rule": "name_pattern", …}}`, no response frames, and no new `Request` entry in `GET …/history`.

## §2 Resolution and discovery (FR-004–FR-006, FR-009–FR-011)

Declare `pick_color` in category `palette` and `echo` in category `memory`. Send:

```json
{"tool":"execute_python","arguments":{"intent":"list client tools","code":"[list_tools(category='palette'), list_tools(category='memory_client'), describe_tool('palette_pick_color')['description']]"}}
```

**Expect**:
- First, `client_tools_resolved` with `palette_pick_color` (`category_name: "palette"`) and `memory_client_echo` (`category_name: "memory_client"`).
- Then the tool result echo, showing both functions, and a description starting `(Provided by the person's client.)`.
- Send the same request twice and compare: both turns' `client_tools_resolved` frames are identical (SC-007's input is stable; the SANDBOX.md lines are pinned by a unit test).

## §3 A call round trip (FR-013, FR-014, FR-017)

Declare `pick_color`. Send:

```json
{"tool":"execute_python","arguments":{"intent":"pick","code":"palette_pick_color(prompt='header')"}}
```

**Expect**: `client_tool_call` with `answer: true`, `tool: "pick_color"`, `arguments.prompt == "header"`. The client answers `{"client_tool_result": {"call_id": …, "ok": {"hex": "#3366ff"}}}`. Then `client_tool_outcome` `ok`, and the reply echo contains `#3366ff`.

**History**: a `ClientToolCall` entry with that uid sits after the turn's `Request` and before its `Response`, with `outcome.status == "ok"`.

## §4 Error, disconnect, timeout, not-pending (FR-015, FR-016; Story 5)

1. Answer with `{"error": "closed"}`. **Expect**: the reply shows a `RuntimeError` containing `closed`, the turn completes, and the outcome is `error`.
2. Run a script with `try: palette_pick_color(prompt='x')\nexcept Exception as e: str(e)` and close the socket on receiving the call. Reconnect and read history. **Expect**: within 5 s the turn has a `Response` whose echo contains `client_disconnected`, and the outcome is `disconnected`.
3. Declare `timeout_secs: 2` and do not answer. **Expect**: `client_tool_cancelled` after about 2 s, the reply contains `timeout`, and the outcome is `timeout`.
4. Send a `client_tool_result` for a made-up call id. **Expect**: `client_error` `not_pending`.

## §5 Only the declaring connection answers (FR-013)

Open a second connection B on the same topic. A declares and triggers a call. **Expect**: A gets `answer: true`, B gets the same call with `answer: false`, and B's `client_tool_result` gets `not_pending` while A's answer completes the call.

## §6 Client events (FR-019–FR-023)

1. From the call in §3, send `{"client_event": {"text": "tools", "data": {"x": 1}, "source_call_id": <id>}}`. **Expect**: a new turn whose reply is the dummyplug tool list. In history, the `Request` entry has `client_event` content with the data and source.
2. Send 11 client events within a minute. **Expect**: the 11th gets `client_error` `rate_limited` and starts no turn.
3. Send a client event whose `source_call_id` is made up. **Expect**: it is delivered, and the stored request has no `source_call_id`.

## §7 Saved state (FR-018a–e; Story 6)

1. Send `{"client_call_state": {"call_id": <id from §3>, "state": {"n": 3}}}` on A. **Expect**: A and B both receive `client_call_state` with `{"n": 3}`, and history shows `state: {"n": 3}` on the entry.
2. Send three saves within 1 s. **Expect**: the third gets `client_error` `rate_limited`, and the stored state is the second.
3. Send a 70 KB state. **Expect**: `state_too_large`, state unchanged.
4. Send state for the `Request` uid of a turn. **Expect**: `not_found`.
5. No new `Request` entries appear from any of the above.

## §8 Turns without a client (FR-007, FR-008, FR-012)

1. Send `{"chat": "tools"}` with **no** declaration and run `list_tools(category='palette')`. **Expect**: an unknown-category error with `did_you_mean`.
2. Trigger a background job from a turn that declared tools (`paralel_subtasks` via dummyplug JSON with one piece). When the report turn runs, read its `Request` entry: **expect** the report text to end with the "Client tools … are not available in this turn" line. The turn's frames contain no `client_tools_resolved`.

## §9 WebUI (browser; Stories 1, 2, 6)

With dummyplug, send in the WebUI:

```json
{"tool":"execute_python","arguments":{"intent":"show a page","code":"ui_render_interactive_ui(title='Hello', html='<button onclick=\"vizier.send(\\'hello\\')\">say hello</button><button onclick=\"vizier.saveState((vizier.state||0)+1)\">count</button><script>vizier.onState(v=>document.title=v)</script>')"}}
```

1. The page appears inside the reply bubble as soon as the call is made, before the tool-result echo, with no border.
2. Click **say hello**: a user bubble reading `hello` (marked "from a page") appears, and dummyplug answers with lorem ipsum.
3. Click **count** three times, then reload. The page shows again with `vizier.state == 3`.
4. Log out, log in on another browser, and open the topic. Same page, same state, and **say hello** still works.
5. With the topic in two tabs, click **count** in one. The other tab's `onState` fires.
6. Run the isolation list from `contracts/webui-extension-api.md` (SC-004) as page scripts. Each attempt fails.
7. `/abort` a turn right after the call: the page stays, and the call shows as `cancelled`.

## Cleanup

Delete the topic. **Expect**: its `client_tool_call` rows are gone. Restarting vizier with a call left pending marks it `interrupted`.
