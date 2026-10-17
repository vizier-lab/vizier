# Quickstart: Scratchpad Tools (dummyplug e2e)

**Feature**: `020-scratchpad-tools`

These steps check the feature against a running binary with an agent on the offline `dummyplug` provider, per the constitution's e2e gate. The dummyplug protocol is `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`. Every step sends one message and checks the reply. All tool calls go through `execute_python` (code mode).

## 0. Setup

1. `just run`, then create an agent with provider `dummyplug` in the WebUI (or `POST /api/v1/agents`).
2. Open a new topic **T1** with it.

## 1. Category is listed (FR-001, FR-015)

Send:

```json
{"tool":"list_tool_functions","arguments":{"category":"scratchpad"}}
```

Expect exactly `write_tmp`, `append_tmp` and `read_tmp`. Sending `tools` shows the four sandbox tools only, as before.

## 2. Write, overwrite, append, read in one turn (US1, US2, SC-001)

In T1, send:

```json
{"tool":"execute_python","arguments":{"code":"print(write_tmp(name='plan', content='step 1'))\nprint(write_tmp(name='plan', content='step 2'))\nprint(append_tmp(name='log', content='a'))\nprint(append_tmp(name='log', content='b'))\nprint(read_tmp(name='plan'))\nprint(read_tmp(name='log'))\nprint(read_tmp(name='nope'))"}}
```

Expect, in order: `written` size 6, `written` size 6, `written` size 1, `written` size 2, `found` `step 2`, `found` `ab`, and `not_found` with `existing: ["log", "plan"]`.

## 3. Refusals are data (FR-010, FR-011, FR-012, SC-005)

```json
{"tool":"execute_python","arguments":{"code":"print(write_tmp(name='../x', content='y'))\nbig = 'x' * 1048576\nprint(write_tmp(name='big', content=big))\nprint(append_tmp(name='big', content='z'))\nprint(read_tmp(name='big')['size'])\nfor i in range(63):\n    write_tmp(name='f' + str(i), content='')\nprint(write_tmp(name='one_more', content=''))\nprint(write_tmp(name='big', content='small'))\nprint('script still running')"}}
```

Expect: `refused/invalid_name`; `written` 1048576; `refused/too_large` with `current: 1048576`; `1048576`; `refused/too_many` (64 files: `big` + 63); `written` size 5 (an overwrite is still allowed); `script still running`.

## 4. Cleared once the conversation is idle (US3, FR-007, SC-002)

After step 3's reply has arrived, send to T1:

```json
{"tool":"execute_python","arguments":{"code":"print(read_tmp(name='plan'))"}}
```

Expect `not_found` with `existing: []`.

## 5. Queued turns share the scratchpad (US3, FR-006)

Over one WebSocket, send these two messages back to back without waiting for a reply. The first loop makes turn A outlast B's arrival.

A:
```json
{"tool":"execute_python","arguments":{"code":"for i in range(3000):\n    append_tmp(name='burst', content='.')\nprint('A done')"}}
```

B:
```json
{"tool":"execute_python","arguments":{"code":"print(read_tmp(name='burst')['size'])"}}
```

Expect B's reply `3000`. Then send B again on its own: `not_found`.

## 6. Isolation (US4, FR-005, SC-003)

Open topic **T2**. Write `draft` = `x` in T1 and `draft` = `y` in T2 within the same script message each, reading it back in the same script. Expect `x` and `y` respectively. Repeat with a second dummyplug agent in its own topic: it never sees the other agent's `draft`.

## 7. Abort clears (FR-007)

In T1, send A from step 5 and then `/abort` before it finishes. Send B: expect `not_found`, or at worst a size from one straggling call (research R4). Never 3000.

## 8. Restart clears (FR-008, SC-004)

1. In T1, send A from step 5. While it runs, stop the server (`just shutdown`, or kill the process).
2. Check `sqlite3 <workspace>/vizier.db "select count(*) from scratch_file"`: it may be non-zero.
3. `just run` again. The log says how many leftover scratch files were cleared. The same query returns `0`, and B in T1 returns `not_found`.

## 9. Deletion (FR-009)

Write a scratch file in T2 from a slow script (A), delete T2 from the WebUI while it runs, then check that `select count(*) from scratch_file where topic = '<T2 id>'` is `0`. Deleting the agent removes all of its rows.

## 10. Dream cycle (FR-016, SC-007)

`cargo test` checks that `write_tmp`, `append_tmp` and `read_tmp` are in `DREAM_TOOL_NAMES`, and that `dream_tools` offers them. With dummyplug the dream prompts produce no tool calls, so the dream run itself is a **live-provider** step: trigger `dream` on an agent with a real model whose CORE asks it to use the scratchpad while dreaming, then confirm `select count(*) from scratch_file where channel like 'DREAM__%'` is `0` after the cycle completes.
