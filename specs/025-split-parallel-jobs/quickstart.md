# Quickstart: Blocking and Background Parallel Subagents (dummyplug e2e)

**Feature**: `025-split-parallel-jobs`

These checks run against a live binary, using an agent on the offline `dummyplug` provider, as the constitution's e2e gate requires. The protocol is `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`. Every tool runs through `execute_python` (code mode).

## How the harness works

- **A piece's prompt is just a message.** `tools` answers with the tool list (§1). `hello` answers with lorem ipsum (§5). A JSON tool request runs that tool inside the piece's turn (§3), so a piece prompt that is an `execute_python` request can sleep (`shell_exec`) or fan out again.
- **Slow pieces need a shell**, so agent `$A` needs a local shell enabled in its tool config.
- **Scripts are shown as Python.** To send one, wrap it as `{"tool":"execute_python","arguments":{"code":"<script, JSON-escaped>"}}` in the WebUI chat input of a topic on `$A`. Inside scripts, this helper builds a piece prompt that runs code:

  ```python
  def q(s):
      return '"' + s.replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\n') + '"'
  def run(code):
      return '{"tool":"execute_python","arguments":{"intent":"run the shell command","code":' + q(code) + '}}'
  ```

- `$T` is the base URL plus an auth header, `$TOPIC` is the topic id and `<ch>` the channel id.

## Setup

```sh
just install && just run-d
```

Create the dummyplug agent `$A` with a local shell enabled and `tools.timeout` left at the default (`30m`). Open a WebUI topic on it.

---

## 1. Both tools are listed and the old name is not (FR-001, FR-002)

```json
{"tool":"list_tool_functions","arguments":{"category":"agents"}}
```

**Expect** `parallel_subagents` and `background_subagents` together with `consult_agent`, `delegate_agent`, `list_background_jobs` and `cancel_background_job`. There is no `paralel_subtasks`.

## 2. The blocking call returns every result in order, in the same turn (US1 AS1, AS2, AS4, FR-004, FR-005, FR-007)

```python
r = parallel_subagents(tasks=[{'prompt': 'tools'}, {'prompt': 'hello'}])
for e in r['results']:
    print(e['task'], e['state'], e['text'][:40], len(e['topic']) > 0)
```

**Expect**:
- One reply, `**Tool result** (execute_python)`, printing `0 answered **Available tools** …` and `1 answered <lorem…>`, both with `True` for the topic.
- No background report entry follows, now or later.
- `curl $T/api/v1/agents/$A/channel/<ch>/topic/$TOPIC/jobs` returns `[]`. `list_background_jobs` returns no job.
- `curl $T/api/v1/agents/$A/channel/SUBAGENT/topic/<topic of entry 0>/history` shows the piece's turn, which ends with the tool list.

## 3. Tasks run in parallel (SC-002)

```python
r = parallel_subagents(tasks=[{'prompt': run("shell_exec(commands='sleep 5')")} for _ in range(3)])
print([e['state'] for e in r['results']])
```

**Expect** `['answered', 'answered', 'answered']`, with the reply about 5 seconds after sending (at most about 6), not about 15.

## 4. A timed-out task keeps its place and stops (US1 AS3, FR-005)

```python
r = parallel_subagents(timeout_secs=3, tasks=[
    {'prompt': 'tools'},
    {'prompt': run("shell_exec(commands='sleep 20')")},
    {'prompt': 'hello'}])
print([(e['task'], e['state'], e['text']) for e in r['results']])
```

**Expect**:
- About 3 seconds later: `(0, 'answered', …)`, `(1, 'timed_out', 'No answer within 3s')`, `(2, 'answered', …)`.
- Piece 1's history (through its `topic`) ends with the abort.
- Nothing more arrives in `$TOPIC` over the next 20 seconds.

## 5. The tool deadline returns what finished (FR-006, research R3)

Set `$A`'s `tools.timeout` to `15s` (agent settings), then:

```python
r = parallel_subagents(tasks=[
    {'prompt': 'tools'},
    {'prompt': run("shell_exec(commands='sleep 60')")}])
print([(e['state'], e['text']) for e in r['results']])
```

**Expect**, about 13 seconds later and not as a tool-timeout error: `('answered', …)` and `('timed_out', 'No answer within 13s')`.

**Too little time left**:

```python
shell_exec(commands='sleep 13')
try:
    parallel_subagents(tasks=[{'prompt': 'hello'}])
except RuntimeError as e:
    print('refused:', e)
```

**Expect** `refused: parallel_subagents: not enough time left in this tool call (2s); …`. Set `tools.timeout` back to `30m` afterwards.

## 6. Aborting the turn stops the pieces (FR-008)

Send a call whose piece sleeps 30 seconds:

```python
parallel_subagents(tasks=[{'prompt': run("shell_exec(commands='sleep 30; echo late')")}])
```

After about 3 seconds, send `/abort`.

**Expect**:
- The turn stops. Its `execute_python` call has a `Stopped:` result in history.
- The piece's history ends with the abort, and `late` never appears anywhere.
- No background report and no further reply in `$TOPIC`.

## 7. A restart mid-wait resumes the turn but not the pieces (FR-008, research R4)

Start the same 30-second call as in step 6, then within about 3 seconds:

- **Update**: change `$A`'s description and save.
- **Crash** (separately): `kill -9` the vizier process, then `just run-d`.

**Expect**, in both cases:
- The parent turn is resumed. Its `execute_python` call carries an `Interrupted:` notice, and the resumed turn replies.
- The piece is **not** resumed. Its history has no steps after the restart, `late` never appears, and the database has no `session_turn` row for the piece's session (`sqlite3 <workspace>/vizier.db "select session from session_turn"`).

## 8. Nesting is bounded across both tools (FR-011)

```python
def wrap(code):
    return "print(parallel_subagents(tasks=[{'prompt': " + q(run(code)) + "}]))"
d3 = "print(parallel_subagents(tasks=[{'prompt': 'hello'}]))"
print(parallel_subagents(tasks=[{'prompt': run(wrap(wrap(d3)))}]))
```

The top-level call runs at depth 0 and its piece at depth 1. Each wrap adds one level, so `d3` makes its call from a depth-3 turn.

**Expect**: the calls at depths 0, 1 and 2 go through. The call `d3` makes is refused with `nesting limit (3) reached`, and that error shows up, nested, inside the outermost result's `text`. Repeat with `background_subagents` as the outermost call: the report's entry carries the same error.

## 9. The background tool is unchanged (US2, FR-010, SC-003)

Re-run `specs/012-background-subagent-results/quickstart.md` §1, §2, §3 and the cancel steps, with `paralel_subtasks` replaced by `background_subagents`, through `execute_python`. **Expect** every result as written there. The acknowledgement still reads `Started background batch b-… with N tasks. …`.

## 10. The retired name explains itself (FR-002, research R8)

```python
try:
    paralel_subtasks(tasks=[{'prompt': 'hello'}])
except RuntimeError as e:
    print(e)
```

**Expect** `paralel_subtasks: paralel_subtasks was replaced by parallel_subagents (waits for the results) and background_subagents (results arrive later as a background report)`. The leading tool name is the sandbox's standard prefix for a catchable tool error. Nothing runs and no job appears.

```json
{"tool":"describe_tool_function","arguments":{"name":"paralel_subtasks"}}
```

**Expect** `available: false` with the same message.

## 11. Guidance and labels (US3, FR-013, FR-014)

- `{"tool":"describe_tool_function","arguments":{"name":"parallel_subagents"}}` and the same for `background_subagents`: the descriptions match `contracts/agent-tools.md`.
- The boot prompt: with `just run-otel`, a turn's `invoke_agent` span (content capture on) shows the "Large tasks" section from the contract.
- `grep -rn paralel_subtasks src docs` finds only `RETIRED_TOOL_NAMES` and its test.
- Usage page: a piece session from step 2 is listed as `Subtask · Subagent`.

## 12. WebUI (FR-015)

- A topic with a `paralel_subtasks` call saved **before** the upgrade (made on a `master` build) still shows the call as a background job launch, with its report.
- Step 2's call shows as an ordinary tool call with its result, not as a job launch.
- A `background_subagents` call shows as a job launch. The tray and the report header read `background_subagents`.
- `cd webui && npm run typecheck && npx vitest run app/lib/trail.test.ts` passes.

## Live-model check (marked: depends on real model output)

On a real provider, ask "compare the licences of serde, tokio and axum" and, separately, "research the history of each of these ten crates in depth". **Expect** the first to use `parallel_subagents` and answer in one reply, and the second to use `background_subagents` and say what it started. This checks the guidance, not the mechanism, so it is advisory.
