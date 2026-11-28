# Research: Triggers

## R1 Where a monitor script runs

**Decision**: the `control` scope schedules checks; the agent's host executes them, through a new
`AgentsClient::run_check` operation. No model call, no conversation, no history.
**Why**: the tool router, MCP clients and secrets live inside the running agent instance, and
`AgentsClient` is the one way to reach it. A check is run like a detached script's `ToolContext`:
non-interactive, no client, `detached: true` (drops turn-ending tools), minus model-backed tools.
**Rejected**: building a router in the scheduler (duplicates MCP connections, breaks isolation);
a queued chat request (would wait behind turns).
**Note**: the contract in `specs/048-agent-service-seam/contracts/agents-client.md` gains one
operation. An unavailable agent makes the check a failure.

## R2 Two transactions

**Decision**: `TriggerStorage::admit` records an event (with delivery id and monitor state) as
`ignored`, `dropped`, `triage_pending` or `waiting`. `TriggerStorage::start_run` checks overlap,
limit and cooldown and moves up to 20 `waiting` events into a new run.
**Why**: concurrent requests must not exceed a cap or start two runs, and FR-027 needs state saved
exactly when an event is accepted. A partial unique index allows one `running` run per trigger.
**Limits**: counted from stored rows, so they survive restarts. A timer in `control` retries
waiting and pending events when a window reopens.

## R3 Closing and recovery

**Decision**: follow task runs: close from the reply stream; after a restart, resume runs whose
session has a pending turn, mark the rest interrupted in `recover_control`, then dispatch any
`queued` row. Writes are fenced on `control`.

## R4 Tool scope

**Decision**: `VizierRequest.tool_scope: Option<Vec<String>>` (category names; `None` = no limit),
copied to `ToolContext`, applied in `turn_catalogue` and `turn_categories`, and copied onto every
request the turn causes (workers, wakes, consultations). Custom tools and nested
scripts inherit the context. The direct sandbox tools are always available. A client cannot set it
(cleared on receipt, as `consultation_chain` is).
**Why**: a name is only dispatched if it is in the turn's catalogue, so one filter covers every
call. Category names cover built-in, addon, MCP and custom categories alike.
**Default narrow scope**: `memory_read`. Vizier delivers results (R13), so no messaging.
**Monitor default**: every category except `discord`, `telegram`, `webui` and `sessions`.
**Consequence**: `sessions` is outside the default, so FR-013's wake needs it added.

## R5 Detached scripts

**Decision**: refuse in a trigger session, beside the scheduled-run and dream refusal. Trigger runs
start at `background_depth` 1.

## R6 Webhook authentication

**Decision**: `POST /api/v1/hooks/{trigger_id}` with `Authorization: Bearer`, or
`/api/v1/hooks/{trigger_id}/{secret}` for senders that cannot set headers. The id is random and
global. Compare in constant time with `ring`; HMAC over the raw bytes. Order: look up, check the
secret, then read the body under a 256 KiB route limit, then check the signature.
**Misses**: one `404` body, rate limited with the existing `client_address` rule.
**Logging**: the secret path segment must never reach logs or spans; add a test.

## R7 Secrets at rest

**Decision**: the webhook token and signing secret are encrypted with `SecretCipher`, using the
field paths a kind marks `Secret`, as integrations do.

## R8 Authorization

**Finding**: `service/tasks.rs` requires only `can_view_agent`, which includes people an agent is
shared with, so reusing it would let a viewer write a script that reads the agent's secrets.
**Decision**: own permissions, `{all_agents,owned_agents}:{view_triggers,manage_triggers,public_webhooks}`,
added to `AVAILABLE_PERMISSIONS` (the startup step already gives new ones to the system role only)
and to `webui/app/lib/permissions.ts`. One rule file, `triggers/access.rs`, as
`custom_tools/access.rs` is. `manage` implies `view`.
**Delivery audience**: notifications today are agent-wide except `chat_reply`. Trigger results and
notices set `audience_user_id` to the trigger's `delivers_to`, extending that to two new categories.
**Model choice**: the existing `model_override` permission.
**Agent tools**: in a WebUI or project chat and in a scheduled run, decided by the turn's
`consulter`. In a Telegram direct message, by the user linked to that account
(`PlatformLinkStorage::find_link`, spec 069); no link means no tools, as does a group. Consultations
and session tools do not change.
**Superseded (2026-11-28)**: this first matched the ids typed into `UserProfile`, which spec 069
replaced with proven links.

## R9 Read-only memory

**Finding**: `memory` includes writing, so a hostile payload could plant instructions in memory or
`core/soul`, which later turns read with all tools.
**Decision**: a scope entry `memory_read` that admits only the reading memory tools (`memory_search`,
`memory_read` and the other non-writing `memory_*` tools, listed by name in one table with a test
that every memory tool is classified). It is a scope entry, not a new `ToolCategory`.
**Left over**: the delivered reply can still repeat data to the trigger's destination.

## R10 Shared sources

**Decision**: `source` is per kind, not per trigger; it reads the active triggers of its kind and
reports firings. This covers one watcher per trigger and one receiver for many.

## R11 Monitor result

**Decision**: the script's value is `None` or `{"state": any, "fire": any?}`; any other shape is a
failed check. The saved state is bound as `input["state"]`.
**Time limit**: wrap the run in a wall-clock timeout; the sandbox's own limit counts CPU only.

## R12 Triage

**Decision**: one completion on the chosen provider and model through the existing model layer, with
a new `UsageScope`, no tools and no history. The answer is a JSON list of
`{event_id, important, reason}`; anything else, or a missing id, makes that event important and
`untriaged`. Payloads are fenced and truncated to a per-call budget. It runs in `control` beside the
source, using the same provider resolution agents use.
**Rejected**: letting the agent judge in a full run (costs the run it is meant to save); a built-in
scoring rule (the filter already covers fixed rules).

## R13 Delivery

**Decision**: when a run closes as answered, claim `delivered_at` in one update, then write the
inbox notification and, if set, post the reply to the destination through the channel's sending path.
At most once: a crash after the claim loses the message but never repeats it; the inbox row keeps the
result. The destination is checked against the setter's access when saved and again at delivery.

## R14 Queue policy per kind

**Decision**: queues and triage are per trigger. `TriggerKind::when_full` picks the full-queue policy,
so `admit` never branches on a kind id. A webhook cannot hold a sender (it was answered), so it
drops the oldest; a monitor can wait, so it refuses and keeps its state.

## R15 Summaries

**Decision**: a timer in `control` builds each due summary from events with `status = unimportant`
and runs with a `quiet_note`, not yet `summarised_at`, as plain text (counts, then up to 50 lines,
then "and N more"). No model call. Delivered like a result, outside the limit.

## R16 Failed runs

**Decision**: closing a run as `no_response` or `interrupted` returns its events to `waiting` with
`attempts + 1`; at 2 they become `failed`. Done in the close transaction.

## R17 Retention and replay

**Decision**: a daily sweep nulls payloads older than 3 days. The optional timestamp check uses the
sender's own header (as GitHub- and Stripe-style schemes provide), compared with a tolerance.
