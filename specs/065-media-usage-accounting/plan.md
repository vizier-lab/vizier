# Implementation Plan: Usage and Spend of Images, Audio and Embeddings

**Branch**: `065-media-usage-accounting` | **Date**: 2026-11-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/065-media-usage-accounting/spec.md`

## Summary

Count and price every image generation, speech synthesis, transcription and embedding request an
agent makes, on the usage page spec 018 built. The four media handles (`VizierImageGen`,
`VizierTts`, `VizierStt`, `VizierEmbedder`) become record points like `VizierModel`: each answered
request writes one `usage_event` row, now carrying a **kind**, its amount in that kind's unit
(images, characters, audio seconds, tokens), whether the amount was reported or measured, whether
the model is local, and the tool call that made it. Provider clients return what the response
reports; the handle measures the rest. Reports price media rows from the catalogue first, then from
a **media price table shipped with Vizier** (`assets/media-prices.json`); local models are free. The
page gains a Kind breakdown and filter, an **Indexing** activity for embeddings made outside any
conversation, and a cost line on chat steps that made media calls.

## Technical Context

**Language/Version**: Rust 2024 (workspace toolchain); TypeScript 5 / React 19 (WebUI)

**Primary Dependencies**: existing only — `tokio` (task-local), `serde_json`, `reqwest` through the
media `Endpoint`; no new crate

**Storage**: `usage_event` gains 8 columns (SQLite `add_column_if_missing`, PostgreSQL
`0016_media_usage.sql`); one state key `usage.media_accounting_since`

**Testing**: `cargo test --workspace`; `cargo test -p vizier-core --no-default-features` and with
`provider-media`; WebUI `npm run typecheck` + vitest for `lib/usage.ts`; end-to-end: dummyplug agent
steps in [quickstart.md](quickstart.md) with the new `dummyplug` media provider, plus one marked
live step

**Target Platform**: server (Linux/macOS/Docker) and the desktop app (incl. Windows)

**Project Type**: Rust workspace (engine library + HTTP crate + binary) with a React WebUI submodule
and a design-system submodule

**Performance Goals**: no perceptible latency on media calls or indexing (SC-004); 90-day report on
100k completion + 500k embedding rows < 2 s (SC-005)

**Constraints**: recording never fails or delays a call (FR-007); no network at startup for prices;
old rows read unchanged (SC-006)

**Scale/Scope**: ~13 media provider clients to report usage; 4 handle types; 1 table; 3 HTTP routes
(2 extended, 1 new); 1 page + 1 trail detail in the WebUI

All unknowns are resolved in [research.md](research.md) (R1–R13).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design — still passing.*

| Gate | Status | How |
|---|---|---|
| I. Lean by default | ✅ | No new crate, no new table; one data file; measuring uses byte/char counts and WAV headers, no audio decoder (R4) |
| II. Trait-based, DRY | ✅ | Recording lives in the four handles behind one shared helper; provider `match` stays in the existing constructors; one pricing function (R1, R6); report path unchanged, `usage_groups` gains columns (R7) |
| III. Self-contained | ✅ | Price table embedded in the binary; no runtime fetch; dummyplug media provider is offline |
| IV. Portability | ✅ | Pure Rust; no platform code |
| V. Errors & observability | ✅ | `crate::Result`; recording failures `tracing::warn!` only |
| Feature flags | ✅ | No new optional capability; media clients stay under `provider-media` where they are; the `dummyplug` media variant is always built (like the dummyplug chat provider) and changes no API shape beyond an added enum value |
| e2e through dummyplug | ✅ | [quickstart.md](quickstart.md) steps 1–11 offline; step 12 marked live |
| Design first | ✅ | FigCraft unreachable this session → ASCII mockup in [contracts/webui.md](contracts/webui.md), to be reviewed before UI code |
| Matching submodule branches | ⚠️ action | Run `just checkout 065-media-usage-accounting` before touching `design/`, `webui/` or `dummyplug/` |
| Compatibility code tracked (#82) | ⚠️ action | Add an #82 entry for the `usage_event` `add_column_if_missing` calls |

No violations; Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/065-media-usage-accounting/
├── spec.md
├── plan.md              # this file
├── research.md          # R1–R13
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── http-api.md      # /usage (kind, by_kind, …), /usage/conversation, /usage/calls
│   ├── media-prices.md  # the shipped price table's format and rules
│   └── webui.md         # ASCII mockup of the page and the step cost line
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
crates/vizier-core/
├── assets/media-prices.json                 # new: the shipped price table (R5)
└── src/
    ├── schema/usage.rs                      # UsageKind, Indexing, UsageEvent fields, report shapes
    ├── agents/agent/model/usage.rs          # UsagePurpose::Media, call_id, MEDIA_SCOPE task-local
    ├── agents/agent/model/mod.rs            # record() → usage::record::spawn_record
    ├── usage/
    │   ├── mod.rs                           # Pricer::media_cost, by_kind, kind filter, units
    │   └── record.rs                        # new: spawn_record + measuring helpers (R4)
    ├── catalogue/media_prices.rs            # new: MediaPrices parse + lookup
    ├── llm/media/{openai_shaped,gemini,cohere,bedrock,ollama,venice,hyperbolic,huggingface}.rs
    │                                        # return Reported from responses (R3) + fixtures
    ├── llm/media/dummyplug.rs               # new: offline media provider (R11)
    ├── image_generation/mod.rs              # VizierImageGen records; trait returns GeneratedImage
    ├── tts/mod.rs, stt/mod.rs               # VizierTts / VizierStt record; Speech / Transcript
    ├── embedding/mod.rs                     # VizierEmbedder records with the ambient scope
    ├── schema/agent.rs                      # Dummyplug variant on the four media provider enums
    ├── agents/tools/{image_gen,tts,stt}/    # pass ctx.usage_scope()
    ├── agents/tools/python/                 # RouterBridge wraps calls in MEDIA_SCOPE; ToolContext.usage_scope
    ├── agents/agent/mod.rs                  # spoken reply + voice-message STT pass the turn's scope
    ├── live_voice/engines.rs                # pass the call's session scope
    ├── storage/usage.rs                     # UsageQuery.kind, UsageGrouping::Kind, media_calls()
    ├── storage/sqlite/{mod,usage}.rs        # columns, grouping, media_calls
    ├── storage/sql/postgres/migrations/0016_media_usage.sql
    └── dependencies.rs                      # set usage.media_accounting_since once
crates/vizier-core/tests/dummyplug_e2e.rs    # media_usage cases
crates/vizier-http/src/api/v1/agents/mod.rs  # kind param, /usage/calls route
design/                                      # breakdown row units line; trail step meta slot (+ stories)
webui/app/
├── routes/usage.tsx, lib/usage.ts           # Kind tab + filter, units formatting, price source
├── api/endpoints/usage.ts                   # /usage/calls
└── screens/chat/                            # step cost line
docs/src/configuration/usage.md              # media accounting, the price table, local = free
```

**Structure Decision**: The existing workspace layout. Engine work in `vizier-core` (record points,
pricing, storage), the route in `vizier-http`, the UI in `design/` then `webui/`. No new crate.

## Phases

- **Phase 0 — research**: done, [research.md](research.md).
- **Phase 1 — design**: done — [data-model.md](data-model.md), [contracts/](contracts/),
  [quickstart.md](quickstart.md).
- **Phase 2 — tasks**: `/speckit-tasks`. Suggested order: schema + storage columns → record helper
  and handles (Story 1) → `Reported` parsing per client → price table + `Pricer::media_cost`
  (Story 2) → report shapes and HTTP (Story 3 backend) → dummyplug media provider and e2e → mockup
  review → design → WebUI page → `/usage/calls` + step cost line (Story 4) → docs.

## Risks

- **Price drift**: the table lags provider changes until a release (accepted in the spec). The
  `as_of` date is shown in the ⓘ tooltip.
- **Embedding volume**: memory-heavy agents write many rows; one row per request (batch), indexed by
  `(agent_id, at)` like today. SC-005 is checked with a seeded 500k-row run.
- **Ambient scope gaps**: an embedding reached from a path that does not set `MEDIA_SCOPE` falls to
  Indexing rather than being lost; the e2e step 5 pins the turn path.

## Complexity Tracking

None.
