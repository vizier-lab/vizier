# Implementation Plan: Triggers

**Branch**: `071-triggers` | **Date**: 2026-11-27 | **Spec**: [spec.md](spec.md)

## Summary

A `Triggers` service in `vizier-core` owns one path: record the event, optionally triage it with a
cheap model, start a run for up to 20 waiting events within the limits, and deliver its reply once.
A run is an `Unattended` request on its own `Trigger` channel, closed the way a task run is.
Kinds sit behind a `TriggerKind` trait: `webhook` receives pushed requests; `monitor` runs one
shared source in the `control` scope and asks the owning agent to execute its script. A tool scope
on the request limits what a trigger run, and everything it causes, may call.

## Technical Context

**Language/Version**: Rust (edition 2024, workspace toolchain); TypeScript/React 19 for the WebUI
**Primary Dependencies**: existing only - `ring` (HMAC, constant-time compare),
axum, monty sandbox. No new crate.
**Storage**: SQLite and PostgreSQL through the shared `SqlStorage` (migration `0020_triggers.sql`)
**Testing**: `cargo test --workspace`; end to end: dummyplug steps in [quickstart.md](quickstart.md)
and `crates/vizier-core/tests/dummyplug_e2e.rs`
**Target Platform**: Linux (x86_64, arm64), macOS, Docker, desktop app
**Performance Goals**: webhook answered within 1 s (SC-002)
**Constraints**: monitor interval >= 5 s; check time <= min(interval, 30 s); body <= 256 KiB;
state <= 64 KiB; no model call in a check
**Scale/Scope**: tens of agents, hundreds of triggers per installation (assumed)

## Constitution Check

| Gate | Result |
|---|---|
| Lean by default | Pass. No new dependency. One trait, justified by two kinds now. |
| Trait-based extensibility | Pass. Nothing matches on a kind id; the webhook route calls `receive`. |
| Single binary, no required service | Pass. |
| Cross-platform | Pass. No OS calls. |
| Dummyplug e2e | Planned (quickstart). |
| Design before UI code | Phase 5 starts with a FigCraft mockup. |
| Feature flags | Not needed: triggers are engine behaviour, not an optional channel or addon. |
| Compatibility code on #82 | None expected: all tables and fields are new. |

## Project Structure

```text
specs/071-triggers/
  spec.md  plan.md  research.md  data-model.md  quickstart.md
  contracts/  trigger-kind.md  http-api.md  agent-tools.md

crates/vizier-core/src/
  triggers/
    mod.rs          NEW  Triggers service: fire, dispatch, close, recover
    kind.rs         NEW  TriggerKind, TriggerKinds::builtin()
    triage.rs       NEW  batching, model call, verdicts, call limit
    delivery.rs     NEW  once per run: inbox and destination; trigger notices
    webhook.rs      NEW  authentication, signature, filter, delivery id
    monitor.rs      NEW  shared source, schedule, backoff
    access.rs       NEW  the three permissions; the person behind a turn
  schema/trigger.rs                     NEW  Trigger, TriggerRun, ToolScope
  storage/trigger.rs, sqlite/trigger.rs NEW  TriggerStorage; admit and start_run
  storage/sql/postgres/migrations/0020_triggers.sql  NEW
  agents/agent/system_prompt/trigger_run.rs          NEW
  agents/tools/triggers/                NEW  agent tools (category appended as the 26th)
  agents/tools/mod.rs                   tool scope in turn_catalogue / turn_categories
  agents/{subagents.rs,tools/sessions,consultations}  carry the tool scope onward
  agents/detached/mod.rs                refuse detach in a trigger run
  agents_client.rs, agents/mod.rs       NEW operation: run_check
  schema/{session,request}.rs           Trigger channel, tool_scope
  storage/history.rs                    is_non_user_channel: trigger__
  integrations/kind.rs                  triggers() default method
  notifications/, usage/, recovery.rs   run close, activity, interrupted sweep
  service/triggers.rs                   NEW
crates/vizier-http/src/api/v1/
  hooks.rs                              NEW  unauthenticated route
  agents/triggers.rs                    NEW
crates/vizier-core/tests/dummyplug_e2e.rs
design/ , webui/                        Phase 5
webui/app/lib/permissions.ts            the six permissions (Phase 2)
docs/src/configuration/triggers.md      NEW
```

**Structure Decision**: a `triggers/` module beside `scheduler/`. The scheduler is not extended:
triggers are not time schedules, and the monitor source owns its own timers.

## Phases

1. **Firing core**: schema, storage, dispatch, close, recovery, framing, usage,
   delivery, tool scope. Tested with a test-only kind.
2. **Webhook**: kind, route, secrets, signature, filter, triage, deliveries, miss limiter, people's
   API, permission. First usable release.
3. **Monitor**: `run_check`, shared source, state, backoff, failure notice, test endpoint.
4. **Agent tools** and the addon hook.
5. **WebUI**: FigCraft mockup and review, then `design/`, then `webui/`.
6. Docs, run the quickstart, update `CLAUDE.md`.

## Risks

- **Scope escape.** Any path that starts a turn without copying the tool scope breaks FR-030. One
  test per path (worker, wake, consultation, custom tool, nested script).
- **Triage swayed or failing.** It fails open, so an outage can spend the run limit (R12).
- **Delivery lost on a crash.** At most once by design; the inbox keeps the result (R13).
- **Platform ids.** Telegram permissions rest on the proven account link of spec 069
  (research R8).
- **Secrets in logs** when carried in the address (R6).

## Decisions (2026-11-27)

- Triggers have their own role permissions (FR-038, research R8).
- The default scope reads memory but cannot write it (FR-030, research R9).

## Complexity Tracking

No violations.
