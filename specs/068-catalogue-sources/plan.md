# Implementation Plan: Catalogue Sources

**Branch**: `068-catalogue-sources` | **Date**: 2026-11-27 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/068-catalogue-sources/spec.md`

## Summary

The catalogue stops being models.dev. It becomes an aggregate of peer **sources**, each
implementing one trait. The sources are models.dev, OpenRouter's lists (text and media, now with
prices), Venice's list (text, image and speech, priced), and the curated table
(`assets/media-prices.json`, now open to text). **Rules** (local and subscription billing) sit on
the provider definition and are asked before any source.

Model facts are merged once per data swap into the same `CatalogueData` shape readers use today,
plus a side table recording which source gave each fact. Prices are answered on demand by walking
the sources in authority order, because prices are kind-scoped and can match by prefix. The walk
also returns the borrowed fallback and any disagreeing prices. `Pricer` loses its own chain and
asks the aggregate. `price_source` becomes a source id. The two existing refresh state machines
become one, run per source, under today's people-only triggers.

## Technical Context

**Language/Version**: Rust 2024 (workspace toolchain); TypeScript 5 / React 19 for the WebUI label
change.

**Primary Dependencies**: existing only: `reqwest`, `serde_json`, `parking_lot`, `tokio`. No new
crate.

**Storage**: SQLite (default) and PostgreSQL. One new table, `catalogue_source_snapshot`
(PostgreSQL migration `0017_catalogue_sources.sql`). `media_catalogue_snapshot` payload schema
goes 1 → 2. No data migration.

**Testing**: `cargo test -p vizier-core` (aggregate merge, authority order, borrowing, disagreements,
each source's parser against its committed seed, SC-005 without models.dev).
`cargo test -p vizier-core --no-default-features` (the catalogue is not behind a flag).
`just web-check` for the label change. End-to-end: dummyplug steps in [quickstart.md](quickstart.md)
(local text priced, borrowed custom-provider price, source labels in the report). One marked live
step uses a Venice key.

**Target Platform**: the server (Linux, macOS, Docker) and the desktop app (macOS, Linux, Windows):
the catalogue is in `vizier-core`.

**Project Type**: Rust workspace (engine library + HTTP crate + binary) with an embedded React WebUI.

**Performance Goals**: the merge of about 11k models after a swap runs in under 100 ms off the
request path (inside the refresh task, or once at startup from seeds). A report prices each
`(provider, model, kind)` once (the existing `Pricer` cache).

**Constraints**: no request at startup, on a schedule, or from agents, the scheduler or dreams
(FR-011, Principle III). A fresh or offline install answers from the shipped seeds. One source's
failure never fails a reader (FR-014).

**Scale/Scope**: 4 sources; about 10k models.dev models, about 550 OpenRouter, about 140 Venice,
24 curated entries. About 15 reader call sites, kept on their current method names.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Gate | Status | How |
|---|---|---|
| I. Lean by default | ✅ | One trait, with four real implementations on day one. No new crate. The OpenRouter media lists and the models.dev refresh share one refresh implementation instead of a third copy. No config flag. |
| II. Trait-based extensibility | ✅ | `CatalogueSource` is the seam. The aggregate never matches on a source id, and billing rules are data on `BuiltinProvider`. Adding a source is one impl plus one line in the registration order (FR-010, SC-005). |
| III. Self-contained default | ✅ | Every source ships a seed. Refresh stays opt-out (`catalogue.refresh`) and people-triggered. No credential is needed for any list. |
| IV. Portability | ✅ | Pure data and HTTP through the existing `reqwest` client. No OS-specific code. |
| V. Unified errors and observability | ✅ | `crate::Result` throughout; a failed source logs with `tracing::warn!` and keeps its last good copy. |
| Feature flags | ✅ n/a | The catalogue is core and always on. No optional capability is added. |
| Design before UI code | ⚠️ pending review | The only UI change (source label, borrowed note, disagreement hint on usage model rows) is mocked in ASCII in [contracts/webui.md](contracts/webui.md). FigCraft was unreachable this session. The mockup has to be reviewed before the WebUI task starts. |
| Conventional commits | ✅ | `feat(catalogue)!:` for the `price_source` rename (breaking wire value). |
| dummyplug e2e | ✅ | [quickstart.md](quickstart.md) scripts it. |
| Migration code to #82 | ✅ n/a | No data is moved. The schema-2 media row simply ignores schema-1 rows (the seed answers). |

**Post-design re-check**: unchanged. The design adds no flag, crate or second implementation of
any existing behaviour.

## Project Structure

### Documentation (this feature)

```text
specs/068-catalogue-sources/
├── plan.md              # This file
├── research.md          # Phase 0
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1 (dummyplug e2e)
├── contracts/
│   ├── catalogue-source.md   # the trait, authority order, merge and price walk
│   ├── seeds.md              # seed files and the parsing of each source
│   ├── http-api.md           # price_source, sources, alternatives, /catalogue status
│   └── webui.md              # usage report labels + ASCII mockup
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/vizier-core/
├── assets/catalogue/
│   ├── api.json                    # models.dev seed (unchanged)
│   ├── openrouter_media.json       # + per-model price (schema 2)
│   ├── openrouter_models.json      # NEW: /api/v1/models, trimmed
│   └── venice.json                 # NEW: text, image, tts lists, trimmed
├── assets/media-prices.json        # curated source; may now hold kind "text"
├── build.rs                        # syntax-check the two new seeds
└── src/
    ├── catalogue/
    │   ├── mod.rs                  # Catalogue = aggregate; readers keep their methods
    │   ├── source.rs               # NEW: CatalogueSource trait, Authority, SourceData, SourceId
    │   ├── aggregate.rs            # NEW: merge(facts) + price walk + borrowing + alternatives
    │   ├── price.rs                # NEW: Price (tokens | per-unit) and PriceAnswer
    │   ├── sources/                # NEW
    │   │   ├── models_dev.rs       #   wraps data.rs + refresh.rs (connection data stays here)
    │   │   ├── openrouter.rs       #   /models + media lists (+ image endpoints)
    │   │   ├── venice.rs           #   text/image/tts list
    │   │   └── curated.rs          #   wraps media_prices.rs
    │   ├── source_refresh.rs       # NEW: the one TTL/backoff/in-progress state machine
    │   ├── builtin.rs              # + Billing on BuiltinProvider
    │   ├── data.rs                 # + ModelKind on CatalogueModel
    │   ├── media.rs                # lists keep serving pickers; price added; refresh moved out
    │   ├── media_prices.rs         # parsing kept; lookup used by curated.rs
    │   ├── refresh.rs              # models.dev fetch only; state moved to source_refresh.rs
    │   └── seed.rs                 # + load the new seeds
    ├── usage/mod.rs                # Pricer asks catalogue.price(); keeps inference profiles
    ├── schema/usage.rs             # price_source: String; price_alternatives; sources
    ├── llm/resolve.rs              # cost read from the aggregate (kept, informational)
    ├── service/catalogue.rs        # status lists every source
    ├── storage/catalogue.rs        # + CatalogueSourceStorage (catalogue_source_snapshot)
    ├── storage/sqlite/mod.rs       # + table
    └── storage/sql/postgres/migrations/0017_catalogue_sources.sql
crates/vizier-http/src/api/v1/catalogue.rs   # status shape (sources[])
scripts/catalogue_update.py                  # fetch the two new seeds + image endpoint prices
webui/app/api/endpoints/usage.ts             # PriceSource → string; sources
webui/app/lib/usage.ts                       # priceNote by source id/name; borrowed; alternatives
webui/app/routes/usage.tsx                   # model row note (after the mockup is reviewed)
docs/src/configuration/usage.md              # where prices come from
docs/src/configuration/providers.md          # catalogue sources section
```

**Structure Decision**: everything stays in `vizier-core/src/catalogue/`, which already owns the
catalogue, its seeds and refreshes. Sources go in a `sources/` folder so each is one file (FR-010).
`vizier-http` changes only the status response shape, and the WebUI only the usage labels.

## Phasing (for /speckit-tasks)

1. **Foundation**: `source.rs`, `price.rs`, `source_refresh.rs`, the aggregate with models.dev and
   curated as the first two sources, `Billing` rules. Readers switch to the aggregate with **no
   figure changing** except local and subscription (US2). This phase is shippable alone.
2. **Provider lists**: OpenRouter (with media prices and image endpoints) and Venice sources,
   seeds, refresh, storage table (US1).
3. **Provenance on the wire**: `price_source` ids, `sources[]`, `price_alternatives`,
   borrowed-from (US3), then the WebUI after the mockup review.
4. **Borrowing** for custom providers (US4) and merged facts in the pickers and warnings (US5).

## Complexity Tracking

No constitution violations to justify.
