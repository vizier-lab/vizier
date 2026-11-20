# Quickstart: verifying consultations and the Workspace removal

These checks run offline against `dummyplug` agents (constitution: e2e through dummyplug). Each
step is a message sent on the WebUI socket or `POST /agents/{id}/prompt`, and its expected
outcome. In code mode every tool is reached through `execute_python`, so a "call" below is
`{"tool": "execute_python", "arguments": {"code": "…"}}` sent to the agent.

A consultee is scripted the same way: the request text A sends **is** a dummyplug message. So
`request: "{\"say\": \"€12,400\"}"` makes B answer `€12,400`, and a request carrying a
`{"tool": "execute_python", …}` JSON makes B run that tool. This is the trick
`scripts/subagent-e2e.mjs` uses.

The automated form of everything below is `node scripts/consultations-e2e.mjs` (which replaces
`workspace-e2e.mjs`) plus the cases in `crates/vizier-core/tests/dummyplug_e2e.rs`.

## Setup

1. Run `just run`, which uses `dev.vizier.yaml`, with no `consultations` key so `max_depth` is 1.
2. Create the dummyplug agents `alpha`, `beta` and `gamma`, all owned by `admin`.
3. Create a user `viewer` who may view `alpha` and `gamma` only (share them to `viewer`).

## 1. Consult and wait (Story 1, SC-001)

1. As `admin`, in a WebUI chat with `alpha`, run:
   ```python
   h = consult_agent(consultations=[{"agent": "beta", "request": "{\"say\": \"pong\"}"}])
   w = wait_consultations(ids=[h["consultations"][0]["id"]], until="all")
   w
   ```
   **Expect**: `w.met == true`, and one snapshot with `state: "answered"`, `result: "pong"`, and
   `consultee.id: "beta"`.
2. `GET /agents/beta/...` conversation list as `admin`. **Expect**: one `kind: "consultation"` row,
   `from_agent.id == "alpha"`.
3. Read that conversation's history. **Expect**: the request's frontmatter has `sender: alpha`,
   `via: consultation`, `on_behalf_of: admin`, `consultation: 1 of 1`. `beta`'s system messages
   end with the `## Consultation` section and contain `beta`'s own BOOT, not the worker framing.
4. `GET /usage` grouped by activity. **Expect**: a **Consultations** row on `beta` and none on
   `alpha` for that consultation.

## 2. Delivery without a wait (Story 1.5, Story 6, SC-004)

1. In `alpha`'s chat: `consult_agent(consultations=[{"agent": "beta", "request": "{\"say\": \"late\", \"delay_ms\": 3000}"}])`,
   and let the turn end.
   **Expect**: within about 4 s a new turn of `alpha` starts with one `[Consultation update]` delivery (`render_update` titles by kind)
   naming `beta` and carrying `late`. No second delivery arrives.
2. Repeat, but abort `alpha`'s turn (`/abort`) right after the call. **Expect**: the same single
   delivery.
3. Repeat, and restart `beta` (`PUT /agents/beta`, an unchanged save) during the delay.
   **Expect**: `beta`'s turn resumes, and the answer is delivered once.

## 3. Stop one, others finish (Story 2, SC-003)

1. Consult `beta` and `gamma`, each with `delay_ms: 5000`. Call `stop_consultation` on `gamma`'s
   id, then `wait_consultations(..., until="all")`.
   **Expect**: `gamma` is `stopped` with `stop_reason` "stopped by the consulting agent", and `beta`
   is `answered`. `gamma`'s conversation shows the turn aborted with a stop notice.

## 4. Asking back and following up (Story 3)

1. Consult `beta` with a request that makes it call
   `ask_consulting_agent(question="eu or us?")`. **Expect**: the wait returns
   `state: "asking"` and `question: "eu or us?"`.
2. Call `reply_consultation(id, text="{\"say\": \"eu it is\"}")`. Wait. **Expect**: `answered`,
   `eu it is`, and `beta`'s history shows the question, the reply (`via: consultation_reply`) and
   the answer in one conversation.
3. Call `reply_consultation(id, text="{\"say\": \"and again\"}")` on the answered row. Wait.
   **Expect**: `answered`, `and again`, and `follow_ups: 1`. The request is framed
   `via: consultation_followup`.
4. Call `reply_consultation` on a `stopped` row. **Expect**: `{status: "not_waiting"}`.

## 5. Access (Story 4, SC-002)

1. As `viewer`, in a chat with `alpha`: `consult_agent(consultations=[{"agent": "beta", "request": "x"}])`.
   **Expect**: `{status: "unknown_agent"}`. The message is identical to the one for
   `{"agent": "nope"}`, and `beta` has no new conversation.
2. As `viewer`: `list_consultable_agents()`. **Expect**: `gamma` only (never `alpha`, never `beta`).
3. Consult `alpha` from `alpha`. **Expect**: `{status: "self"}`.
4. A scheduled task of `alpha` consults `beta`. **Expect**: success (the owner `admin` may view
   `beta`), and `on_behalf_of: none`.
5. In a Discord or Telegram conversation with `alpha` (or with the reader stubbed, a
   `discord__…` session sent through `POST /agents/alpha/prompt`'s test path in
   `dummyplug_e2e.rs`), run `list_tool_functions(category="consultations")`. **Expect**: an
   unknown category (`did_you_mean` data). The same holds for a detached script launched there
   (FR-011a).
6. As `viewer`, list `gamma`'s conversations. **Expect**: no consultation rows (`viewer` cannot
   edit `gamma`).

## 6. Limits (FR-015 to FR-017, SC-005)

1. Have `beta`'s scripted request run `list_tool_functions(category="consultations")`.
   **Expect**: only `ask_consulting_agent`.
2. In a subagent worker of `alpha`, list `consultations`. **Expect**: the category is absent.
3. In `beta`'s consultation, check for `ask_question`. **Expect**: absent.

## 7. Deletion (FR-022, SC-007)

1. With a running consultation of `beta`, delete `alpha`'s conversation. **Expect**: the row is
   gone, `beta`'s consultation conversation is gone (history, files, scratch), and `beta`'s turn
   was aborted.
2. With a running consultation of `gamma`, delete `gamma`. **Expect**: `alpha` gets one delivery,
   `stopped` with "the consulted agent was deleted".

## 8. People's view (Story 5, Story 7)

1. While a consultation is asking, open `alpha`'s chat. **Expect**: the strip shows
   **Consultations 1** with an amber dot, the panel row shows the question, and expanding a
   finished row draws `beta`'s steps with the chat's Trail.
2. Open `beta`'s consultation conversation as `admin`. **Expect**: no composer. A `chat` frame
   sent on its socket gets `read_only_conversation`.
3. `GET …/consultations/{id}` for another person's topic. **Expect**: `404`.

## 9. Workspaces removed (Story 8, SC-006)

1. Before upgrading, seed a data set with a Workspace (two topics, two agents, messages,
   notifications). Upgrade and start. **Expect**: the log shows the cleanup once (marker
   `migration.drop_workspaces_v1`), no `workspace*` table remains, the agents have no
   `workspace__…` sessions, and their memory, other conversations and `usage_event` rows are
   unchanged. The usage report still lists the old Workspace activity as "Workspaces (removed)".
2. `GET /api/v1/workspaces`. **Expect**: `404`.
3. The WebUI rail shows no Workspace group, and `+` opens New agent directly.
4. In any chat:
   ```python
   mention_agent(agent="beta", message="hi")
   ```
   **Expect**: a catchable error whose text names `consult_agent` and `wait_consultations`. The
   same holds for `send_message_to_workspace`, `list_workspaces`, `read_workspace_messages` and
   `delegate_agent`.
5. A Discord or Telegram group message with no mention. **Expect**: it is silently read as before.

## Checks

Run each of these and confirm it passes:

```powershell
cargo test --workspace
```

```powershell
cargo clippy --workspace
```

```powershell
cd webui; npm run typecheck; npx vitest run
```

```powershell
node scripts/consultations-e2e.mjs
```
