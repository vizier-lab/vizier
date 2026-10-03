# Quickstart: Task Run Results, Requester and Framing

**Feature**: `011-task-completion-reports` · **Date**: 2026-10-04

End-to-end verification against a running binary with an agent on the offline `dummyplug`
provider, per the constitution's e2e gate. No credentials, no network.

Two things about the harness shape, stated up front so the steps below make sense:

- **The scheduler ticks once a minute** (`Duration::from_mins(1)`, `scheduler/mod.rs:39`), so
  every step that waits for a fire waits up to ~70s. Nothing here is instant.
- **Dummyplug answers a scheduled task prompt with lorem ipsum** (protocol §5), so a dummyplug
  run reliably reaches `answered` with non-empty text. That makes the happy path deterministic,
  and it means the *content* of a report cannot be judged here — only that one exists. The
  report-versus-reply behaviour needs a real model and is marked as such at the end.

## Setup

```sh
just install && just run-d          # or: cargo run -- run --config dev.vizier.yaml -d
```

Create a dummyplug agent through the WebUI or the API (no provider entry needed — protocol
"Provider API"). Below, `$A` is its id and `$T` the base URL with an auth token set.

---

## 1. A fired one-time task survives, with a result (FR-001, FR-002, FR-004)

```sh
# two minutes out, so the next tick catches it
curl -X POST $T/api/v1/agents/$A/tasks -d '{
  "slug":"oneshot","title":"One shot",
  "instruction":"Say something about the weather",
  "schedule":{"type":"OneTime","datetime":"<now+2min, RFC3339>"}}'
```

Wait ~2 minutes, then:

```sh
curl $T/api/v1/agents/$A/tasks/oneshot
```

**Expect**: `200`, not `404` — this is the regression that started the feature. `is_active` is
`false`. `last_run.state` is `"answered"`, `last_run.response` is the lorem text, and
`last_run.ran_at` is set.

**Also expect** the task to stay fired: wait another two minutes and confirm `last_run.ran_at` is
unchanged and no second run appears (FR-002).

---

## 2. Each run gets its own conversation (FR-003, Decision 2)

```sh
curl -X POST $T/api/v1/agents/$A/tasks -d '{
  "slug":"tick","title":"Tick",
  "instruction":"Note the time",
  "schedule":{"type":"Cron","expression":"* * * * *"}}'
```

Let it fire three times (~3.5 min), then:

```sh
curl "$T/api/v1/agents/$A/tasks/tick/runs?limit=10"
```

**Expect**: three entries, newest first, each with a distinct `run_id`, and **no `response`
field anywhere in the listing** (FR-014/FR-020).

**The regression this guards**: before the slug fix every run collided into `task__tick__0`.
Three distinct `run_id`s is the whole point. Confirm separation directly:

```sh
curl "$T/api/v1/agents/$A/tasks/tick/runs/<run_id of the 1st>/history"
curl "$T/api/v1/agents/$A/tasks/tick/runs/<run_id of the 3rd>/history"
```

**Expect**: each returns only its own exchange — one request and one response each, not three
runs' worth. A growing history on the later run means the slug fix did not take.

---

## 3. Pagination neither duplicates nor skips (FR-007 – FR-009)

With `tick` left running for ~10 minutes:

```sh
curl "$T/api/v1/agents/$A/tasks/tick/runs?limit=3"
# then, from the oldest entry of that page:
curl "$T/api/v1/agents/$A/tasks/tick/runs?limit=3&before=<its ran_at>&before_id=<its id>"
```

**Expect**: no `run_id` appears on both pages, and the run immediately older than the page
boundary is the first entry of the second page. `has_more` is `true` until the last page, then
`false` (FR-009).

**Mid-paging insert** (FR-008): page once, wait for a new run to land, then request the second
page with the original cursor. The new run must appear on neither page — the cursor is anchored
to where the first page ended, not to an offset.

---

## 4. A firing that produced nothing is still recorded (FR-024, FR-005)

```sh
curl -X POST $T/api/v1/agents/$A/tasks -d '{
  "slug":"orphan","title":"Orphan",
  "instruction":"Anything",
  "schedule":{"type":"OneTime","datetime":"<now+2min>"}}'

curl -X DELETE $T/api/v1/agents/$A       # agent gone before the task fires
```

Wait past the scheduled moment, then read the run list for `orphan`.

**Expect**: one run, `state: "no_response"`, `finished_at` set. The firing is visible rather
than absent — a task that has been failing must not look like one that never ran.

**Expect also**: `last_run` present with `"response": null`, which is distinct from a never-run
task's `last_run: null` (FR-005).

---

## 5. Never-run and running are distinct states (FR-005)

Create a task scheduled well into the future and read it: `last_run` is `null`.

For `running`, create a minutely task and poll `…/runs?limit=1` tightly across a tick boundary.
A dummyplug reply is fast, so the window is short — a run observed with `state: "running"` and
`finished_at: null` is the confirmation. If it is missed, Step 9's unit coverage stands in.

---

## 6. Deleting a task takes its runs with it (FR-012, FR-013)

```sh
curl -X DELETE $T/api/v1/agents/$A/tasks/tick
curl "$T/api/v1/agents/$A/tasks/tick/runs"        # expect 404
```

Then recreate a task reusing the slug `tick`, and before it fires:

```sh
curl $T/api/v1/agents/$A/tasks/tick
```

**Expect**: `last_run: null`. The new task must not inherit the deleted one's runs.

---

## 7. The requester is recorded and shown (FR-027 – FR-033)

**Created over HTTP** — post a task with an extra `"user":"somebody-else"` in the body:

```sh
curl $T/api/v1/agents/$A/tasks/<slug>
```

**Expect**: `requester` is `{"user":"<the authenticated caller>"}`, **not** `somebody-else`
(FR-028). The supplied field is ignored, not rejected — this is the breaking change.

**Created by the agent** — through dummyplug, run the scheduling tool by hand (protocol §2/§3):

```text
schedule_one_time_task
```

then send back the JSON sample with `requester` set to the agent's own id, and separately with a
person's identity. **Expect** both to be accepted and to read back as `{"agent": …}` and
`{"user": …}` respectively. A chat-channel identity with no account must be accepted (FR-029).

**Migration** (FR-031): against a data directory from before this change, start the binary and
confirm every pre-existing task reads back with `requester: {"user": "<its old user value>"}` and
none is lost.

---

## 8. An agent reads its own task reports (FR-019 – FR-026)

Through dummyplug, by hand:

```text
tools
```

**Expect** `list_task_runs` and `get_task_run_detail` in the list. Then:

```text
list_task_runs
```

and send back the sample JSON with a slug that has runs.

**Expect**: runs newest first, each with `run_id`, `ran_at` and `state`, and **no response text**
(FR-020). Then `get_task_run_detail` with one of those `run_id`s:

**Expect**: that run's `response`, and **no reasoning or tool activity** (FR-022). Confirm
`response` is `null` for a `no_response` run.

**Scoping** (FR-025): send a slug belonging to a different agent's task — expect a refusal.

**Dream availability** (FR-026): trigger a dream cycle and confirm both names are in the tool set
offered during it. Note the cycle only runs when there were conversations in the window
(`scheduler/dream/mod.rs:37`), so chat with the agent first or the cycle returns immediately.

---

## 9. Checks that are not dummyplug steps

These are pure functions or process-lifecycle behaviour; `cargo test` is the right home and the
honest place to say so.

| Check | Where |
|---|---|
| `VizierChannelId::Task.to_slug()` yields a distinct key per timestamp, and a legacy `__0` key still parses | unit, `schema/session.rs` |
| Framing is selected for `Task`, and **not** for `Dream` or `HTTP` — the trap in Decision 8 | unit, `system_prompt/` |
| The framing string is constant across runs (FR-033) | unit |
| Page cursor over runs sharing a millisecond | unit, `storage/sqlite/task_run.rs` |
| Startup sweep turns `running` into `interrupted` (FR-018) | unit on the sweep, plus one manual check: kill the process mid-run (`just shutdown` during a fire), restart, confirm the run reads `interrupted` and the task is not permanently blocked |
| One `running` row per task at a time (FR-010) | unit |

---

## 10. Report versus reply — **requires a live provider**

Marked per the constitution: this depends on real model output, which dummyplug cannot produce
(protocol §5 returns lorem ipsum regardless of the prompt).

Point an agent at a small, inexpensive model. Give it an ordinary, slightly underspecified task —
*"Summarise what changed in the repo today and post it to the team channel"* — and let it fire.

**Expect** its response to read as a report: it leads with the outcome, states any assumption it
made, and contains no question addressed to a reader and no offer to act later (SC-014, FR-029).

**Before/after is the real test here.** Run the same instruction with the framing disabled and
compare. The pre-feature failure is specific and recognisable: a greeting using the requester's
name, or a clarifying question followed by a stop. If the response is indistinguishable either
way, the framing is not reaching the model — check the selection is on the channel and not on the
request content kind.

**Also confirm** no regression elsewhere: an interactive chat turn with the same agent is
unchanged (FR-032), and a dream cycle's extraction output is unchanged (FR-031).

---

## Gates

```sh
cargo clippy
cargo test
cd webui && npm run typecheck
```
