# Quickstart: Background Subagent Results

**Feature**: `012-background-subagent-results` · **Date**: 2026-10-09

This is end-to-end verification against a running binary, using agents on the offline `dummyplug` provider, as the constitution's e2e gate requires. It needs no credentials and no network.

## How the harness works

These steps rely on four facts about dummyplug (protocol `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`):

- **A piece's prompt is just a message.** A subtask prompt of `tools` answers with the tool list (§1). A prompt that is a JSON tool request **runs that tool inside the piece's turn** (§3). That is how the steps below build slow pieces, failing pieces and nested jobs without a real model.
- **A report's text is neither `tools` nor JSON**, so the woken turn answers with lorem ipsum (§5). The steps check that a woken reply *exists* and where it went, not what it says. How a real model responds to a report is a live-model check, marked at the end.
- **Slow pieces use `shell_exec`**, so agent A needs a local shell enabled in its tool config.
- **Run tool JSON from the WebUI chat input** of a topic on agent `$A`. `$T` is the base URL with an auth header, and `$TOPIC` is the topic id.

## Setup

```sh
just install && just run-d
```

Create two dummyplug agents, **`$A`** (local shell enabled) and **`$B`**. Open a WebUI topic on `$A`.

---

## 1. A batch returns immediately and reports back once (US1, FR-001, FR-005–FR-007)

Send:

```json
{"tool":"paralel_subtasks","arguments":{"tasks":[{"prompt":"tools"},{"prompt":"hello"}]}}
```

**Expect**:

1. Within about 1 second, the reply reads `**Tool result** (paralel_subtasks): Started background batch b-…… with 2 tasks. …` (SC-001).
2. Without sending anything else, a **background report** entry appears, collapsed, with `2 ✓`. A new agent reply (lorem) follows it.
3. `curl $T/api/v1/agents/$A/channel/<ch>/topic/$TOPIC/history`: the newest `Request` has `content.background_report` with two entries in order. Entry 0 is `answered` with the tool list as its text. Entry 1 is `answered` with lorem.
4. `curl …/topic/$TOPIC/jobs` returns `[]`, and `curl …/jobs/<id>` returns `state: "reported"`.

## 2. The conversation stays live while a batch runs (US1 AS3, FR-006, SC-002)

Send a batch whose single piece sleeps for 20 seconds:

```json
{"tool":"paralel_subtasks","arguments":{"tasks":[{"prompt":"{\"tool\":\"shell_exec\",\"arguments\":{\"commands\":\"sleep 20; echo done\"}}"}]}}
```

Immediately send `tools`.

**Expect**:

- The tool list comes back at once, not after 20 seconds.
- The tray above the input shows `⟳ Batch b-… 0/1 done` with elapsed time counting up.
- About 20 seconds later, the report arrives (entry 0: `answered`, containing `done`) followed by a woken reply, and the tray clears.

**Queued variant**: launch the same batch, then at second 18 send a JSON that runs `sleep 5` directly in the main turn. The report must arrive **after** that turn's reply, never in the middle of it (US1 AS4).

## 3. Time-outs keep their place, late answers are discarded (US4, FR-013, SC-004)

```json
{"tool":"paralel_subtasks","arguments":{"timeout_secs":3,"tasks":[
  {"prompt":"tools"},
  {"prompt":"{\"tool\":\"shell_exec\",\"arguments\":{\"commands\":\"sleep 20\"}}"},
  {"prompt":"hello"}]}}
```

**Expect**:

- After about 3 seconds there is one report with three entries in order: `answered`, then `timed out` ("No answer within 3s"), then `answered`.
- 20 seconds later, **no second report** has arrived.
- The piece's history (`…/jobs/<id>/pieces/1/history`) ends with the abort.

## 4. Delegation reports back, naming the other agent (US2, FR-008)

On `$A`:

```json
{"tool":"delegate_agent","arguments":{"agent_id":"<B>","prompt":"tools"}}
```

**Expect**:

- The acknowledgement names the job and `$B`.
- The report's title is `delegation b-… to <B>`, and its single entry is `answered` with `$B`'s tool list.
- `…/pieces/0/history` shows `$B`'s side of the conversation.

Then:

```json
{"tool":"delegate_agent","arguments":{"agent_id":"nobody","prompt":"x"}}
```

**Expect** a tool error `agent 'nobody' not found or not running`, no report, and no job in `…/jobs` (US2 AS2).

## 5. A conversation with no person gets the report in history only (US3 AS4, FR-011)

On `$A`, delegate to `$B` a prompt that itself launches a batch on `$B`:

```json
{"tool":"delegate_agent","arguments":{"agent_id":"<B>","prompt":"{\"tool\":\"paralel_subtasks\",\"arguments\":{\"tasks\":[{\"prompt\":\"tools\"}]}}"}}
```

**Expect**:

- `$A` gets a report whose text is `$B`'s *acknowledgement*. This is the documented limit: nested results report to the conversation that launched them, not the top level.
- `$B`'s piece session (from `…/pieces/0/history`) then holds `$B`'s own background report followed by a reply.
- Nothing appears on any person-facing channel for `$B`.

## 6. The nesting limit stops chains (FR-012, SC-006)

Generate a prompt that nests `paralel_subtasks` four levels deep, then paste the output into the chat:

```sh
p='"tools"'
for i in 1 2 3 4; do
  p=$(jq -cn --argjson inner "$p" '{tool:"paralel_subtasks",arguments:{tasks:[{prompt:($inner|if type=="string" then . else tojson end)}]}}')
done
echo "$p"
```

**Expect**: the reports cascade back up. The innermost launch, from a turn at depth 3, fails with `background nesting limit (3) reached`, and that error text is the answer shown in its parent's report entry. No turns continue after the last report.

## 7. WebUI tray (US5)

During step 2's 20-second piece:

| Check | Expect |
|---|---|
| Expand the job | One piece row: `⟳ {"tool":"shell_exec"… · running · 0:07 ›` |
| Click `›` | The side panel shows the piece's trail with the `shell_exec` call, and fills in when the piece finishes |
| Click `↥ jump` | The conversation scrolls to the `paralel_subtasks` tool row |
| Reload mid-run | The tray comes back with the same job and elapsed time (FR-016, SC-008) |
| Narrow the window below 640px | The tray becomes a `⟳ 1 running` pill that opens a bottom sheet |
| Switch to another topic mid-run | That topic's list entry shows a `⟳1` badge after the topic list refreshes |
| When the job finishes | `✓ 1 answered` shows for about 3s, then the tray clears. The report entry and the woken reply survive a reload |

## 8. Restarts (Decision 9, FR-020)

**Agent respawn**: start step 2's batch, then within 20 seconds change any setting on `$A` and save it. That respawns the agent process.

**Expect**: a report with entry 0 `failed` ("the agent was restarted"), delivered to the respawned agent.

**Server restart**: start step 2's batch with the tray open, then run `just shutdown && just run-d`.

**Expect**:

- `curl …/jobs/<id>` returns `state: "interrupted"`.
- The open tray, after it reconnects, shows `⚠ lost` for about 3s, then clears.
- No report is ever delivered.

## 9. Discord and Telegram are woken but nothing is posted (US3 AS3, FR-010)

This step needs a Discord or Telegram bot token but no live model. On a dummyplug agent with a Discord token, mention the bot with step 1's JSON.

**Expect**:

- The acknowledgement is posted (that is the normal reply to your message).
- After the batch finishes, **nothing further is posted** to Discord: no report and no woken reply.
- The agent's history for that Discord session (WebUI → Agent → sessions, or the history API) holds the `background_report` request followed by the woken reply.

Repeat on Telegram.

## 10. Cancelling (US6, FR-024–FR-029, SC-010)

**By the agent.** Launch a batch with one slow piece and one fast one:

```json
{"tool":"paralel_subtasks","arguments":{"tasks":[{"prompt":"tools"},{"prompt":"{\"tool\":\"shell_exec\",\"arguments\":{\"commands\":\"sleep 60\"}}"}]}}
```

Once the tray shows `1/2 done`, send:

```json
{"tool":"list_background_jobs","arguments":{}}
```

**Expect**: one job listed, with piece 1 `answered` and piece 2 `running`. Then send, using that id:

```json
{"tool":"cancel_background_job","arguments":{"job_id":"b-…","reason":"test"}}
```

**Expect**:

- The tool result is `Cancelled background batch b-… (reason: test)`, followed by entry 1 `answered` with the tool list and entry 2 `cancelled`.
- Within 5 seconds the tray shows `⊘ cancelled` and clears, and `…/pieces/1/history` ends with the abort.
- 60 seconds later, **no report** has arrived.
- `curl …/jobs/<id>` returns `state: "cancelled"` and `cancelled_by: {"agent": "<A>"}`.

**Repeat cancel.** Send the same cancel JSON again. **Expect**: `Background job b-… already finished (cancelled).`

**Cascade.** Launch step 6's nested prompt, but with the innermost `"tools"` replaced by a JSON that runs `shell_exec` `sleep 60`. While it runs, cancel the outer job. **Expect**: `Also cancelled nested jobs: …` lists the inner jobs, and `curl …/jobs/<inner>` returns `cancelled` for each.

**Foreign job.** On agent `$B`, call `cancel_background_job` with a job id launched by `$A`. **Expect**: `no running job b-… launched by you`.

**From the tray.** Launch step 2's slow batch, click `✕` → `Cancel job`. **Expect**:

- The tray shows `⊘ cancelled` and clears.
- The conversation shows `⊘ You cancelled background job b-…`.
- The agent produces **no** new reply.
- `cancelled_by` is `{"person": "<you>"}`.

**Race.** Launch a batch with `timeout_secs: 2` and a 60-second piece. At about second 2, cancel from the tray. **Expect** exactly one outcome: either a report arrives and the tray's cancel gets `409`, or the job is `cancelled` and no report arrives. Never both.

## Live model only

With a real provider, ask: "Research these three topics in parallel and summarise." The agent should:

- launch `paralel_subtasks`;
- tell you it has started instead of making up results;
- after the report arrives, post one summary that mentions every topic, including any that failed.

This depends on model behaviour and is not deterministic.
