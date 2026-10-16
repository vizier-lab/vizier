# Quickstart: Complete Usage Accounting

These are end-to-end checks against a running binary, per the constitution's dummyplug gate. Agent steps use the dummyplug protocol (`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`). Tools are called from code mode:

```json
{"tool":"execute_python","arguments":{"intent":"…","code":"print(…)"}}
```

dummyplug reports **zero tokens** (research R11). The dummyplug sections therefore check calls: their count, activity, place, model, served model, the failed marking and the links. Token and cost arithmetic is checked by unit tests, plus the one live step in §8.

## Build and static checks

```sh
cargo clippy && cargo test
cd webui && npm run typecheck
```

Unit tests cover:
- `VizierChannelId::usage_activity()` and `place` for every variant;
- `estimate_cost(&TokenCounts, …)` with cache inside and outside input, compared with the old figures for the same stats;
- grouping and pricing over seeded `usage_event` rows: the totals, `previous`, `share_of_all`, `unpriced`, `cache_hit_rate`, and failed via `usage_failed_turn`;
- `migrate_reply_usage`: seeded `session_history` rows give the same `summary` as the old `aggregate_usage` did (SC-007), and it is idempotent on a second run;
- `TurnGuard`: dropping it unmarked records a failed turn, and `succeeded()` records nothing;
- the insight template's threshold (50%).

## Fixtures

- Agent `dp` on `dummyplug`, with `dream_interval` set (dreams enabled).
- `read_image` configured with vision provider `dummyplug` and model `vision-dp`, so vision calls have a model distinct from the chat model.
- One image uploaded as a session file in WebUI topic `T1`.
- Helper: `U(q)` = `GET /api/v1/agents/dp/usage?{q}` for today's range.
- Before §1, note `U()` → `summary.calls` as `C0`.

## 1. Chat turns are counted, per round (US1, FR-001–FR-003)

| # | Action | Expect |
|---|---|---|
| 1 | In `T1`, send `hello` | `U()` → `calls = C0 + 1`. `by_activity` has `chat`, and `by_model` has `dummyplug/<agent model>`. |
| 2 | In `T1`, send a tool request: `{"tool":"execute_python","arguments":{"intent":"x","code":"print(1)"}}` | `calls` grows by **2**: the tool round and the final round. `U(activity=chat)` → `by_activity = []` and `summary.share_of_all` is set. |
| 3 | `GET …/usage/conversation?channel=<T1 slug>&topic=<T1>` | `calls` lists rounds `1` and `2`, `activity: chat`, `failed: false`, and `link.kind = chat` with `path = /dp/chat/<T1>`. |

## 2. Vision (US1 sc. 5, US2)

| # | Action | Expect |
|---|---|---|
| 1 | In `T1`, run `read_image_file` on the uploaded image from `execute_python` | `by_activity` gains `vision`, and `by_model` gains `dummyplug/vision-dp`, separate from the chat model. |
| 2 | Details for `T1` | It includes a `vision` call with `model: vision-dp`. |

## 3. Compaction (US1 sc. 2–3)

| # | Action | Expect |
|---|---|---|
| 1 | In `T1`, send the `checkpoint` command | `by_activity` gains `compaction` with 1 call, and the place is still `T1`'s topic. |

The automatic checkpoint can't be checked offline: it trips on reported input tokens, and dummyplug reports zero. It is covered live in §8.2. Both paths go through the same `generate_handover_with_model` call, so step 1 covers the offline case.

## 4. Dreams (US1 sc. 1)

| # | Action | Expect |
|---|---|---|
| 1 | `POST /agents/dp/dream/trigger` | After it finishes, `by_activity` has `dreams` with one call per extraction plus the consolidation (as many as the dream journal shows for that cycle), and `by_place` has a `dreams` row labelled "Dream cycle". |
| 2 | Details for one dream session | `link.kind = dream`, `path = /dp/dream`. |

## 5. Background and scheduled (FR-003)

| # | Action | Expect |
|---|---|---|
| 1 | In `T1`, run `paralel_subtasks` with 2 pieces, each just `hello` | `by_activity` gains `background` with 2 calls, plus the report turn as `chat` in `T1` (R4). Details of a piece's session give `link.kind = topic`, pointing at `T1`. |
| 2 | Create a one-time task due in 1 minute (`POST /agents/dp/tasks`) whose prompt is `hello`; wait for it to run | `by_activity` gains `scheduled`. `by_place` has `task:<slug>`, labelled with the task title. Details give `link.kind = task_run`. |

## 6. Failed and aborted turns (US1 sc. 4, FR-004)

| # | Action | Expect |
|---|---|---|
| 1 | Set `dp`'s `prompt_timeout` to 2s. In `T1`, send a tool request whose script busy-loops for about 5s, e.g. `for i in range(30000000): x += i` | The turn fails with a timeout. In details for `T1`, the round that completed before the timeout is `failed: true`, and `summary.failed.turns` grows by 1. |
| 2 | Restore `prompt_timeout`. Start a busy-loop turn in `T1`, then `PUT /agents/dp` with a changed description while it runs (the update aborts its turns, #52) | The aborted turn's completed round is `failed: true`. |
| 3 | Delete topic `T1` | `U()` totals are unchanged (FR-008). Its `costliest` entry has `exists: false`, and details give `no_link_reason: deleted`. |

## 7. Carry-over and the page (FR-007, FR-019, SC-007)

| # | Action | Expect |
|---|---|---|
| 1 | Against a data dir copied from before the upgrade: note the old page's totals, upgrade, start | `U()` for the same range shows the same `summary.tokens`, `calls` and `cost_usd`. `accounting_since` is the startup time, and a second start doesn't change it. |
| 2 | Open `/dp/usage` in the WebUI | It shows the partial-accounting notice for 30 days. `?activity=dreams` filters every section and hides the Activity dimension. **Details** opens the drawer, and its link opens the conversation. Esc closes the drawer and returns focus. |
| 3 | At phone width (390px) | No horizontal page scroll. The tables scroll inside their own boxes, and the drawer is full width. |
| 4 | Delete agent `dp` | Its `usage_event` and `usage_failed_turn` rows are gone (`sqlite3 … 'select count(*) from usage_event where agent_id="dp"'` → 0). |

## 8. Live provider (marked: needs a key, real model output)

| # | Action | Expect |
|---|---|---|
| 1 | An agent on a priced catalogue model; send one message | `summary.tokens > 0`, `cost_usd` is not null, and `by_model[0].served_models` holds the provider's dated version. |
| 2 | Same agent, long enough conversation to trip `checkpoint_threshold` | One `compaction` call after the turn (step 3.2). |
| 3 | An agent on a custom provider with no prices | Its tokens appear in `unpriced`, and `cost_usd` excludes them. |
| 4 | Timing: send 20 messages with and without the feature on the same build | Turn latency is unchanged within noise (SC-005). |
