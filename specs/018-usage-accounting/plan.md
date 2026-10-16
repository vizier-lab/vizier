# Implementation Plan: Complete Usage Accounting and an Insightful Usage Page

**Branch**: `018-usage-accounting` | **Date**: 2026-10-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/018-usage-accounting/spec.md`

## Summary

Every language-model completion made for an agent is recorded as its own usage event, at the moment it is answered. That includes dreams, checkpoint handovers, rounds of failed or aborted turns, and vision reads, none of which are counted today. Each event records provider, requested and served model, tokens, activity and conversation. The usage page is rebuilt on top of those events: one headline cost with its change against the previous period, an activity filter, one daily chart, one switchable ranked breakdown, the costliest conversations, and a details drawer for each.

Technical approach:
- `VizierModel`, the one agent-level funnel all four call sites already use, records each answered call. A required `UsageScope` argument says what the call is for (R1).
- Writes are fire-and-forget, so recording never delays a turn (R2).
- Failed turns are marked by a drop guard writing to a separate table, which also covers aborts (R3).
- The activity is decided once, at write time, from the scope and `VizierChannelId::usage_activity()` (R4).
- A new `UsageStorage` trait sits over two SQLite tables (R5). A one-time startup carry-over copies existing reply stats so totals before the upgrade are unchanged (R6).
- SQL groups the rows and Rust prices them with the existing catalogue pricing, generalised to `TokenCounts` (R7).
- `GET …/usage` changes shape `[**breaking**]`, and `GET …/usage/conversation` is added (R8).

## Technical Context

**Language/Version**: Rust 2024 edition (stable); TypeScript 5 / React 19 (WebUI)

**Primary Dependencies**: existing only: tokio, rusqlite (bundled), axum, serde, utoipa, uuid, chrono; React Router v7, Tailwind v4, Recharts. No new crates or npm packages.

**Storage**: embedded SQLite. New tables `usage_event` and `usage_failed_turn` (data-model.md §2), and a state key `usage.accounting_since`. One startup migration, `migrate_reply_usage`, copies existing reply stats into the new table.

**Testing**: `cargo test` covers the activity and place mapping, `estimate_cost` parity, grouping and pricing over seeded rows, carry-over parity and idempotence, the `TurnGuard`, and the insight threshold. Also `cargo clippy` and `npm run typecheck`. End-to-end: dummyplug agent steps in [quickstart.md](quickstart.md), checking calls, activities, models, failed marking and links. The token, cost, auto-checkpoint and latency steps are marked live (R11).

**Target Platform**: the single vizier binary (Linux/macOS/Windows; musl builds unchanged)

**Project Type**: single Rust binary with an embedded web app (`src/` + `webui/`)

**Performance Goals**:
- recording adds no awaited work to a turn (SC-005);
- a 90-day report over about 100k events loads in under 2 seconds (SC-006);
- the details call is bounded by `limit` (≤ 100).

**Constraints**:
- recording never fails or alters a call (FR-006);
- one record per answered call, never one per retry attempt (FR-005);
- usage survives session deletion and dies with the agent (FR-008);
- no network on a person's read beyond what exists today (names come from storage, R9);
- `src/llm/` stays agent-agnostic.

**Scale/Scope**: about 15 Rust files touched or added, 1 migration, 2 endpoints, 1 route rewritten and 2 components added. Audio, TTS, STT, image generation and embeddings are out of scope (FR-009).

## Constitution Check

*GATE: checked before Phase 0 and again after Phase 1 design.*

| Principle | Check | Status |
|---|---|---|
| I. Lean by Default | No new crates and no new long-lived subsystem (writes are spawned, R2). One trait (`UsageStorage`) because storage concerns are traits by convention. No config flags. The recorder-channel option was deferred until contention shows up. | ✅ |
| II. DRY via traits | Recording happens in one place (`VizierModel`), not in each of the four callers. The activity mapping lives on `VizierChannelId` next to `to_slug()`/`platform()`, not in call sites. Pricing stays one function (`estimate_cost`, generalised). `aggregate_usage` is removed rather than kept next to the new path. | ✅ |
| III. Self-contained runtime | Embedded SQLite only. Readable names come from storage, with no live Discord or Telegram lookups. The WebUI stays embedded. | ✅ |
| IV. Portability | No platform-specific code. | ✅ |
| V. Unified errors & observability | Storage returns `crate::Result`. Recording failures go to `tracing::warn!`. No `unwrap` outside tests. `TurnGuard::drop` uses `Handle::try_current()` instead of panicking. | ✅ |
| Gate: e2e through dummyplug | quickstart.md scripts §1–7 offline. The live steps (§8) are marked, as the gate requires for token accounting. | ✅ |
| Gate: conventional commits / breaking | The `GET …/usage` response change ships as `feat: …[**breaking**]`. | ✅ |
| Gate: manual verification for storage migrations | Quickstart §7.1 runs the binary against an older data dir. | ✅ |

**Post-design re-check (after Phase 1)**: no violations. Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/018-usage-accounting/
├── plan.md              # this file
├── research.md          # Phase 0: R1–R12
├── data-model.md        # Phase 1: types, schema, UsageStorage, report types
├── quickstart.md        # Phase 1: dummyplug e2e script (+ marked live steps)
├── contracts/
│   ├── http-api.md      # GET …/usage (changed), GET …/usage/conversation (new)
│   └── webui.md         # usage route, details drawer, activity palette
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks; not created here)
```

### Source Code (repository root)

```text
src/
├── schema/
│   ├── usage.rs                 # NEW: UsageActivity, UsageEvent, UsageReport & row types
│   ├── metrics.rs               # TokenCounts; estimate_cost generalised; old AgentUsageStats types removed
│   ├── session.rs               # VizierChannelId::usage_activity(), ::usage_place()
│   └── mod.rs                   # re-exports
├── agents/
│   ├── agent/
│   │   ├── model/
│   │   │   ├── mod.rs           # VizierModel: agent_id, complete(req, &UsageScope), spawn record
│   │   │   └── usage.rs         # NEW: UsageScope, UsagePurpose, TurnId, TurnGuard
│   │   └── mod.rs               # prompt/dream_prompt: TurnGuard + per-round scope; handover scope; new_with_override(agent_id)
│   ├── process.rs               # manual checkpoint passes the session to the handover scope
│   └── tools/
│       ├── mod.rs               # vision model built with agent_id
│       └── read_image/mod.rs    # Vision scope from ToolContext.session
├── storage/
│   ├── usage.rs                 # NEW: UsageStorage trait + VizierStorage forwarding
│   ├── mod.rs                   # add UsageStorage to VizierStorageProvider
│   ├── history.rs               # aggregate_usage removed from HistoryStorage
│   ├── fs/history.rs            # aggregate_usage impl removed
│   └── sqlite/
│       ├── usage.rs             # NEW: inserts, grouped queries, conversation query, delete_agent, tests
│       ├── mod.rs               # init_usage_schema
│       ├── history.rs           # aggregate_usage removed
│       └── agent.rs             # delete_agent → usage::delete_agent
├── usage/                       # NEW: report building (pricing groups, previous period, labels, links, insight)
│   └── mod.rs
├── dependencies.rs              # migrate_reply_usage (R6)
└── channels/http/
    ├── api/v1/agents/mod.rs     # agent_usage → UsageReport; new usage/conversation route
    └── mod.rs                   # OpenAPI schema list

webui/app/
├── routes/usage.tsx                         # rewritten (contracts/webui.md)
├── components/UsageDailyChart.tsx           # NEW (replaces UsageBarChart.tsx)
├── components/UsageConversationDrawer.tsx   # NEW
├── components/usageActivities.ts            # NEW: palette + labels
├── interfaces/types.ts                      # new usage types; old removed
└── services/vizier.tsx                      # getAgentUsage(…, activity), getConversationUsage
```

**Structure Decision**:
- The report-building logic (pricing grouped rows, the previous period, label and link resolution, the insight template) goes in a small `src/usage/` module. Both HTTP handlers call it. It needs storage plus the catalogue, so it belongs in neither the storage backend nor the HTTP handler.
- Everything else extends existing modules in place.

## Complexity Tracking

No constitution violations to justify.
