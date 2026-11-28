# Handover: Triggers (071)

**Date**: 2026-11-28 · **Branch**: `071-triggers` (local only) · **Issue**:
[vizier-lab/vizier#220](https://github.com/vizier-lab/vizier/issues/220)

## Status

Phases 1 to 5 are built: the backend, and the Triggers page in `design/` and `webui/`. All of it is
one local commit on `071-triggers`, not pushed, rebased on 2026-11-28 onto upstream master
`8b00ba7e`. Upstream had taken spec number 068 and migration 17, so this spec is now **071** and its
PostgreSQL migration **0020**. Spec 069 replaced typed platform ids with proven links, which the
trigger tools now use; offering them on Discord is not looked at again yet.

| Phase | State |
|---|---|
| 1 Firing core | Done |
| 2 Webhook | Done |
| 3 Monitor | Done (skipping a check that overruns its interval has no test) |
| 4 Agent tools, addon hook | Done; not offered on Discord |
| 5 WebUI | Done; checked by hand against a seeded demo server |
| 6 Finish | `CLAUDE.md` updated; quickstart run except steps 15 and 16 |

## Read first

[spec.md](spec.md) (behaviour), [plan.md](plan.md), [research.md](research.md) (why),
[data-model.md](data-model.md), [contracts/](contracts/), [tasks.md](tasks.md),
[quickstart.md](quickstart.md). User docs: `docs/src/configuration/triggers.md`.

## Done on 2026-11-28

| Fix | Notes |
|---|---|
| Runs resumed after a restart are closed | `open_trigger_runs`; a restart test that fails without it |
| Agents cannot widen a monitor's tools | A new or rewritten script gets `memory_read`; an unchanged one keeps a person's grant; `test_trigger` refuses while awaiting approval and never exceeds the turn's scope |
| Scope on delivery turns (FR-030) | Table `trigger_work_scope`; replies to workers never widen it; per-path tests |
| An empty tool scope allows nothing | It used to allow everything |
| Removed triage model (FR-042) | Marked `needs_attention` and told once, found when the next event is triaged |
| People cannot save triage with no model | |
| Operator settings (FR-025, 029, 048) | `triggers:` in the config file |
| Control gate (FR-039) | Triage and runs start only where it is open; intake still records |
| Oversized body (FR-022) | `413` only after the secret |
| Lost updates | Targeted writes for firing time, summary time and attention |
| Expired delivery ids | Deleted by the daily purge |
| WebUI tool list | Trigger tools named on the activity trail; `scripts/trail-tool-categories.test.mjs` was failing and passes |
| `webui/openapi.json`, `app/api/schema.d.ts` | Regenerated |
| Trigger storage tests | Also run on PostgreSQL now |

## Gaps left

| Gap | Requirement |
|---|---|
| Agent tools not offered on Discord (the spec records this). Since spec 069 a request carries the sender's platform id, so a linked Discord sender could now be named: not done | FR-041 |
| A result delivered *into a turn already running* keeps that turn's scope, not the work's. A decision to confirm | FR-030 |
| A triage model the provider itself rejects (name retired, provider still set up) fails open but is not marked `needs_attention` | FR-042 |
| No generic long-running source: only the built-in monitor has one, so an addon kind can receive pushes but not pull (the trigger-kind contract says so) | FR-004 |

## Not verified

- **Real services**: no real webhook sender, Telegram bot or paid model.
- Quickstart steps 15 and 16 (need a real bot). It ran on SQLite only, with stand-in models: see
  [quickstart.md](quickstart.md). The docs site was not built.
- `just check` as one command: the `vizier` binary needs `cmake`.
- S3 storage tests (no MinIO).
- Speech feature (`provider-media`) left out: `cmake` is not installed.

## Test results (2026-11-28, after the rebase)

| Suite | Result |
|---|---|
| `vizier-core` unit | 2374 passed, 8 ignored, on SQLite and again with PostgreSQL |
| `dummyplug_e2e` | 36 tests; `a_restart_mid_turn_reads_the_message_exactly_once` failed once, then passed 9 times |
| `triggers_e2e` | 11 passed |
| `lsp_host` | 25 passed, 3 failed; the same 3 failed on untouched upstream `0b708d63`, not rechecked on `8b00ba7e` |
| `agent_host`, `lsp_real`, `roles` | passed |
| `vizier-http`, `vizier-schema`, `vizier-fleet` | 198 passed |
| `webui` typecheck, vitest, build | clean; 1786 passed, 1 skipped; builds |
| `design` typecheck, Storybook build | clean; builds |
| `design` vitest | 26 failed in 10 files, the same count as on `0b708d63`; CI does not block on it |
| `node --test scripts/*.test.mjs` | 11 passed |
| Clippy (CI lints) | clean |
| Quickstart | before the rebase: 22 of 24 steps on SQLite with stand-in models; 15 and 16 not run |

The page has not been looked at in a browser since the rebase.

## Run it

Use the stable toolchain (the project needs Rust 1.96; the default here is 1.94.1). The
language-server tests need their fixture built first.

```sh
cargo +stable build -p vizier-core --example fake_lsp
```

```sh
cargo +stable test -p vizier-core --no-fail-fast --features channel-discord,channel-telegram,addon-docker,addon-documents,utoipa,telemetry
```

```sh
cargo +stable test -p vizier-http
```

PostgreSQL tests need a database with the `vector` extension and `VIZIER_TEST_DATABASE_URL` set.
`node_modules` is installed in `design/` and `webui/`, so building `vizier-http` also builds the WebUI.

## Where the code is

Under `crates/vizier-core/src/`:

| Path | Holds |
|---|---|
| `triggers/mod.rs` | accept, triage-then-run loop, dispatch, finish, resume, tick, redaction |
| `triggers/{kind,webhook,monitor}.rs` | the kind trait, webhook checks, monitor source |
| `triggers/{triage,delivery,access}.rs` | verdicts, results and summaries, permissions |
| `storage/{trigger.rs,sqlite/trigger.rs}` | `admit_event`, `start_trigger_run`, `close_trigger_run` |
| `agents/tools/scope.rs` | tool scope rules, `intersect`, `memory_read` |
| `agents/delivery.rs` | delivery sections carry their work's scope |
| `agents/tools/triggers/` | the nine agent tools |
| `service/triggers.rs` | people's operations and authorization |

HTTP: `crates/vizier-http/src/api/v1/hooks.rs` (public address) and `agents/triggers.rs`.

## Decisions to keep

- Results go only to the person who set the trigger up; the agent's owner is the fallback.
- Every trigger an agent creates waits for approval. The agent never sees a webhook secret;
  approval issues a fresh one.
- Default scope: webhooks and agent-made triggers read memory only; a person's monitor gets all
  tools except messaging and waking.
- A script an agent writes or rewrites starts narrow, even on a monitor a person had widened.
- 20 runs an hour per trigger, counting only important events; adjustable or off.
- Triage is optional, any configured model, and fails open.
- A Telegram sender is identified by the account link of spec 069, never by a typed profile id.
- Each kind picks its full-queue policy: a webhook drops the oldest, a monitor refuses the new one.

## Remaining work, in order

1. Finish T043: `just check` where `cmake` is installed (or in CI), and the quickstart on
   PostgreSQL.
2. Push to the fork, and discuss the differences from the issue with its author.

## The page

`webui/app/routes/{triggers,trigger,trigger-edit}.tsx`, `screens/triggers/`, `lib/triggers.ts`,
`api/endpoints/triggers.ts`, `stores/triggers.ts`. In `design/`: a `zap` icon, a count on `NavItem`
and `ContextSidebar`, and a compact `ProgressBar`.

The drawing follows the build (decided 2026-11-28): the mockup frames were redrawn from the page's
code and screenshots. What that settled, all from using existing components:

- The kind is chosen with radios, tool access with switch chips, and a list shows "Latest" without a
  run's duration.
- A run shows its reply and events in place: the chat reader cannot open a trigger conversation, so
  there is no "Open conversation".
- The approval page chooses the triage model in a dialog from its Approve button.
- The empty state stacks a button per kind inside one dashed box.
- The side panel is one card with rules between sections.

A saved signing secret cannot be removed from the form: a blank secret keeps it. Sending `null`
removes it through the API.

Checked by hand: list, webhook and monitor pages, create with its secret, test, restart, approve
with a model, edit, a view-only person, phone width and the light theme. Not checked: replace
secret, delete, a monitor created from the form, destinations, the desktop app.

## The mockup (2026-11-28)

Page `Triggers · mockup` of `Vizier shell.figcraft` (outside the repo): list, empty, new
webhook, webhook detail, stopped monitor, approval, secret shown once, triage model dialog and notes.
The redrawn frames are saved.

- A `Triggers` sidebar item after Schedule; a task-style detail page with settings in a side panel.
- Desktop dark only. Not drawn: view-only and no-permission states, edit and the monitor form,
  confirm dialogs, a test result, loading and errors, phone, light theme. The notes frame says how
  they behave.
- The frames keep the file's drawn shell, which is narrower on the left than the running app's.

The server side of the mockup is done: a trigger is read with `latest_run`, `runs_last_hour` and
`needs_person`, and `GET /agents/{id}/trigger-summary` counts them for the heading and the sidebar
badge.

One `vizier-http` test failed once on 2026-11-28 and passed in the five runs after; its name was not
captured. `--test triggers` alone passed 25 times in a row afterwards. One possible race, not
reproduced: the monitor source can check the new test's monitor while the test writes its record.
