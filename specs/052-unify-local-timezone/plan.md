# Implementation Plan: Unified Local Time

**Branch**: `052-unify-local-timezone` | **Date**: 2026-11-06 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/052-unify-local-timezone/spec.md`

## Summary

Vizier runs one clock: the host's timezone. A single `Timestamp = DateTime<Local>` replaces `DateTime<Utc>` across the crates, so stored JSON, the API, tool results and cron all become local together, while old `Z` values still read. Text columns used for sorting stay in UTC behind one helper, so ordering holds across offsets. The `## Time` context section goes, and four read-only tools in a new `time` category replace it. Existing one-time tasks are rewritten once, keeping their moment; cron tasks and dream schedules are never rewritten, and now mean local time. The WebUI shows the server's zone regardless of the browser's. Startup refuses a bad `TZ`, including the case where chrono would silently fall back to UTC.

## Technical Context

**Language/Version**: Rust (workspace edition, chrono 0.4.45); TypeScript, React 19 (WebUI)

**Primary Dependencies**: `chrono` (`Local`), `croner` 4 (generic over `TimeZone`), `iana-time-zone` (in tree). **New direct dependency**: `chrono-tz` 0.10 (already in both lockfiles, [R5](research.md#r5-chrono-tz-becomes-a-direct-dependency-of-vizier-core)). `tracing-subscriber` gains its `chrono` feature.

**Storage**: SQLite and PostgreSQL through the shared `SqlStore`. No schema change. One data migration on `task.data`, plus a change to `Dialect::day_of_millis`.

**Testing**: `cargo test --workspace`, repeated with `TZ=Asia/Jakarta`; zone logic is tested with `chrono_tz` zones through functions generic on the zone; vitest with a zone unlike the runner's; conformance vectors. End-to-end: dummyplug steps in [quickstart.md](quickstart.md).

**Target Platform**: Linux and macOS servers, Docker, desktop app on macOS, Linux and Windows

**Project Type**: Multi-crate server with an embedded WebUI and a desktop host

**Performance Goals**: None new. `Local::now()` is cached by chrono; a turn makes no extra model calls unless the agent wants the time.

**Constraints**: A UTC instance's output stays byte-identical. Existing data stays readable with no downtime. The system prompt and context stay byte-stable across turns.

**Scale/Scope**: About 480 `Utc` references in 137 Rust files (mechanical), about 15 text columns, about 10 WebUI modules.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Rule | Status | Notes |
|---|---|---|
| I. Lean by Default | Pass | An alias rather than a newtype; no config key. One new dependency, justified in R5 |
| II. DRY via traits | Pass | One `time` module for parsing, formatting and day logic. Time tools use `VizierTool`; `day_of_millis` stays a `Dialect` method |
| III. Self-contained default | Pass | Zone data is compiled in for names; no service added. The default path works with no `TZ` |
| IV. Portability | Pass | `chrono-tz` is pure Rust. The Unix-only `TZ` check is isolated in `time::init_zone`. `musl` and Windows are covered by R4 |
| V. Unified errors and observability | Pass | `crate::Result`, `tracing`; startup failure reports through `VizierRuntime::start` |
| Dependency review | Pass | Version already in tree. `app/Cargo.lock` stays aligned (CI `policy`) |
| Migrations tracked on #82 | Required in tasks | The rewrite, its marker, the ignored `tz`, and the query-space leniency |
| Design first | Required in tasks | The timezone indicator is drawn in FigCraft before code. Other WebUI changes display existing elements, so no design pass |
| Matching submodule branches | Required in tasks | `just checkout 052-unify-local-timezone` was not run by the branch hook. Commit order: design → webui → vizier |
| Dummyplug e2e | Pass | [quickstart.md](quickstart.md) scripts it; live-provider steps are marked |
| Runtime verification | Required in tasks | Startup, migration and `Dockerfile` changes must be run, including `cross build` for the targets |
| Flags and roles | Pass | No optional capability added; `vizier-schema` and `vizier-fleet` carry no time types |

**Post-design re-check**: all still pass, and no violation needs justifying. See [R2](research.md#r2-text-columns-keep-a-fixed-sort-form-fr-006) for the one deliberate narrowing of FR-003.

## Project Structure

### Documentation (this feature)

```text
specs/052-unify-local-timezone/
├── plan.md              # This file
├── research.md          # Decisions R1–R13
├── data-model.md        # Types, stored values, migration state
├── quickstart.md        # Dummyplug verification script
├── contracts/
│   ├── time-tools.md    # The four tools and the scheduling tool changes
│   └── http-api.md      # New endpoint, wire format, deprecated tz
└── tasks.md             # Created by /speckit-tasks
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── time.rs                          # NEW: Timestamp, now, Zone, init_zone, parse_schedule, stored, day helpers
├── runtime.rs                       # Calls time::init_zone first
├── dependencies.rs                  # Runs the one-time task migration
├── agents/
│   ├── agent/system_prompt/
│   │   ├── context.rs               # Remove ## Time
│   │   ├── boot.rs                  # Rewrite the Time section
│   │   └── reactions.rs             # Timestamp, no behaviour change
│   └── tools/
│       ├── time.rs                  # NEW: the four tools
│       ├── mod.rs                   # ToolCategory::Time, DREAM_TOOL_NAMES, category table
│       └── scheduler/mod.rs         # Local input, descriptions, confirmations
├── scheduler/                       # Timestamp; cron becomes local through the type
├── service/{overview,dream,tasks}.rs # Local days, ignore tz, parse_schedule
├── storage/
│   ├── sql/{sqlite,postgres}.rs     # day_of_millis in the local zone
│   └── sqlite/*.rs                  # time::stored for text sort columns
└── channels/{discord,telegram}/     # sent_at local

crates/vizier-http/src/api/v1/
├── utils.rs                         # GET /utils/timezone
└── overview/, agents/dream.rs       # tz deprecated, ignored

crates/vizier/src/
├── main.rs                          # Local log timer
└── cli/agent.rs                     # Retry time local, no UTC label

webui/app/
├── lib/timezone.ts                  # NEW: zone store, zoned, wallClockString
├── lib/{format,conversations,schedule,dreams,overview,messages}.ts
├── routes/{home,task-edit}.tsx
└── screens/schedule/, screens/shell/ # Zone gate in AppShell, indicator

Dockerfile                           # tzdata in the runtime image
scripts/052-utc-check.sh             # NEW: refuses a new Utc outside the allow-list
.github/workflows/ci.yml             # policy job runs the script
docs/src/configuration/time.md       # NEW + SUMMARY: setting the zone, cron behaviour, tools
CLAUDE.md                            # Time section; remove UTC wording
```

**Structure Decision**: Engine logic goes in `vizier-core`, the one route in `vizier-http`, and the log timer in the `vizier` binary, following the workspace's one-way dependency rule. `vizier-schema` is a leaf and untouched.

## Implementation Order

Each step leaves the tree building and tests passing.

1. **`time` module** with its unit tests: resolver, `parse_schedule`, day helpers, `stored`.
2. **Sweep** `Utc` to `Timestamp`, with text columns through `stored`, and `init_zone` in the runtime. Audit serialized-session keys (R2). A UTC instance is byte-identical at this point.
3. **Day boundaries**: `day_of_millis`, overview and dream days, `tz` ignored.
4. **Scheduling input and tools**, then the migration.
5. **Prompt**: the `time` tools, then remove `## Time` and rewrite boot. These land together, so an agent is never left with neither.
6. **WebUI**: the endpoint and `lib/timezone.ts`, then the screens. FigCraft → design → webui for the indicator.
7. **Edges**: logs, CLI, `tzdata`, the check script in CI, docs, CLAUDE.md, #82 entries, release notes.

## Risks

| Risk | Mitigation |
|---|---|
| An upgrade shifts cron firing times | Decided by the request; R3 shows at most one catch-up; release notes and docs state it |
| A value compared as text somewhere the audit misses | R2 audit; quickstart step 9; `052-utc-check.sh` stops new cases |
| SQLite's `localtime` disagrees with chrono on an unusual host | Both read the same OS setting; the R4 self-check fails early when zone data is missing |
| A client posts `before=` with an unencoded `+` | Server accepts the decoded form ([contract](contracts/http-api.md#changed-times-in-urls)) |
| An agent guesses the date instead of asking | The boot section says it does not know; quickstart step 13 checks it on a live model |
| Fleet nodes in different zones | Assumed the same in the spec; flag it when #166–#170 wire the roles |

## Complexity Tracking

No constitution violations to justify.
