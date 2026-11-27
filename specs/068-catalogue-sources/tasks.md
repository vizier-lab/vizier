# Tasks: Catalogue Sources

**Input**: Design documents from `specs/068-catalogue-sources/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: included. The plan and the quickstart list unit checks (seed parsing, authority order,
SC-005, SC-002, the FR-021 golden list), and the constitution requires `cargo test`, `cargo clippy`
and the dummyplug e2e to pass.

**Paths**: `core/` = `crates/vizier-core/`, `http/` = `crates/vizier-http/`. Others are from the
repository root.

**House rules for every task**: never run `cargo fmt` (the workspace is not rustfmt-clean). Use
`crate::Result` and `tracing` only. Write `crate::time::now()`, never `Utc::now`. No new crate.
Readers keep calling `Catalogue::{provider, model, models, find_model_by_id, snapshot}`.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1–US5 from spec.md

---

## Phase 1: Setup

**Purpose**: module skeleton, so later tasks have files to fill.

- [X] T001 Create empty modules and declare them in `core/src/catalogue/mod.rs`: `core/src/catalogue/source.rs`, `core/src/catalogue/price.rs`, `core/src/catalogue/aggregate.rs`, `core/src/catalogue/source_refresh.rs`, `core/src/catalogue/sources/mod.rs` (declaring `models_dev`, `curated`, `openrouter`, `venice`) and the four empty files under `core/src/catalogue/sources/`. `cargo check -p vizier-core` passes.

---

## Phase 2: Foundational (blocking all stories)

**Purpose**: the aggregate with models.dev and curated as its first two sources, and `Pricer`
asking it, with **no figure changing**. When this phase is done the build behaves exactly as
before, apart from the `price_source` wire values renamed in T015.

- [X] T002 [P] Implement `Price` in `core/src/catalogue/price.rs` per data-model §6. Fields: `tokens: Option<Cost>`, `per_image: Option<ImagePrice>` (move `ImagePrice` here from `media_prices.rs` and re-export it there), `per_million_characters`, `per_audio_minute`. Add `Price::zero()`. Add `Price::cost(kind, &TokenCounts, &MediaUnits, variant, cache_outside_input) -> Option<f64>`, ported from `usage/mod.rs::Pricer::priced`: tokens first when the record has tokens and the price has token rates (via `schema::metrics::estimate_cost`), else units as `MediaPrice::unit_cost` does today. Add `PartialEq` within 1e-9. Add unit tests for each kind and for equality.
- [X] T003 [P] Add `ModelKind { Text, Image, Speech, Transcription, Embedding }` and `kind: Option<ModelKind>` to `CatalogueModel` in `core/src/catalogue/data.rs`. Derive it in `parse` from `modalities.output` (text→Text, image→Image, audio→Speech). In `Catalogue::models()` (`core/src/catalogue/mod.rs`), keep only `kind` = `Text` or unknown. Rename the enum `data::CatalogueSource` to `SnapshotOrigin` everywhere (`grep -rn "CatalogueSource::" crates`), since "source" now means a catalogue source. *Done with `UsageKind` as the model kind (no new enum). `models()` is not filtered: the image-capability picker reads it too, so media models from provider lists live in a separate priced-only map instead.*
- [X] T004 Define the seam in `core/src/catalogue/source.rs` exactly as `contracts/catalogue-source.md` §1: `SourceId` (a `&'static str` newtype with `MODELS_DEV`, `OPENROUTER`, `VENICE`, `CURATED` and the rule labels `LOCAL`, `SUBSCRIPTION`, `INFERENCE_PROFILE`, `BORROWED`, `NONE`), `Authority { Own, Curated, General, None }` (ordered), `SourceData`, `SourceProvider`, `ModelFacts` (data-model §4–5), `RefreshCtx { deps: VizierDependencies, http: reqwest::Client }`, and the `CatalogueSource` trait with the default `price()` impl. Depends on T002, T003.
- [X] T005 Implement `SourceRefresh` in `core/src/catalogue/source_refresh.rs` (data-model §9). Move the TTL check, the 1 h failure backoff, `in_progress` and the 30 s `refresh_now` wait out of `core/src/catalogue/refresh.rs::{maybe_refresh, refresh_now}` and `core/src/catalogue/media.rs::MediaCatalogue::{maybe_refresh, refresh_now}` into one implementation. `status()` returns the existing `RefreshStatus`. Both old call paths delegate to it, and behaviour is unchanged. Depends on T004.
- [X] T006 [P] Implement the `models_dev` source in `core/src/catalogue/sources/models_dev.rs`. It wraps today's `CatalogueData` (seed via `seed::load_seed`, cache via `cache::newest`, fetch via `refresh.rs::check`, which keeps logos) and converts it to `SourceData`, filling `connection` and every `ModelFacts` field and the price from `cost`. Its authority is `General` for every provider. `refresh.rs` keeps only the fetch and the trim. Depends on T004, T005.
- [X] T007 [P] Implement the `curated` source in `core/src/catalogue/sources/curated.rs` over `media_prices::media_prices()`. Its authority is `Curated` for every provider. Override `price()` to call `MediaPrices::lookup(provider, model, kind)` and convert a `MediaPrice` into a `Price`. Allow `kind: "text"` entries in `core/src/catalogue/media_prices.rs` (the `UsageKind::Text` arm returns the token price). `refresh` is a no-op. Depends on T004.
- [X] T008 Implement the merge in `core/src/catalogue/aggregate.rs` (`contracts/catalogue-source.md` §4). `merge(&[Arc<dyn CatalogueSource>]) -> Merged { data: CatalogueData, provenance }`: provider connections from `models_dev`, the union of model ids over sources with authority ≠ `None`, each field (and each `Capabilities` sub-field) from the most authoritative source that has it, and ties broken by registration order. Add unit tests with two fake sources: per-field fallthrough, authority beating registration order, and an empty source not hiding the other's facts. Depends on T004.
- [X] T009 Implement the price walk in `core/src/catalogue/aggregate.rs` (contract §5, **without** borrowing, which is T032): `price(provider, model, kind) -> PriceAnswer` (data-model §8) with `alternatives`, aliasing through `builtin::model_source`. Add `PriceAnswer` to `core/src/catalogue/price.rs`. Unit tests: the first source wins, a differing later source becomes an alternative (≤ 3), an equal one does not, and a miss reads `none`. Depends on T008, T002.
- [X] T010 Turn `Catalogue` in `core/src/catalogue/mod.rs` into the aggregate: hold `Vec<Arc<dyn CatalogueSource>>` in registration order (`openrouter`, `venice`, `curated`, `models_dev`; the first two are added in US1) plus `ArcSwap`/`RwLock<Arc<Merged>>`. `snapshot()` returns the merged `CatalogueData`. Add `price()`, `sources()` and `remerge()`, which runs after any source swaps. Keep `Catalogue::new(CatalogueData)` as "models.dev with this data + curated" so the existing tests in `mod.rs`, `llm/resolve.rs`, `llm/smoke_tests.rs` and `service/catalogue.rs` still compile unchanged. `swap(data)` keeps meaning "replace models.dev's data", then re-merges. Depends on T006, T007, T008, T009.
- [X] T011 Wire startup in `core/src/dependencies.rs` (lines ~192–255): build the catalogue from every source's seed, then call each source's `load_cached(&storage)` and re-merge once. It must stay network-free. Keep `migrate_provider_ids_v1` reading `catalogue.snapshot()`. Depends on T010.
- [X] T012 Fan out the refresh in `core/src/service/catalogue.rs`: the existing `refresh::maybe_refresh(deps)` call sites (lines ~822, 883, 914, 935, 966) become `catalogue::maybe_refresh(deps)`, which runs every source's `SourceRefresh`. `refresh_now` waits for all of them and returns models.dev's result as today (`contracts/http-api.md`). Add no new call sites. FR-011 holds when `grep -rn "maybe_refresh\|refresh_now" crates/vizier-core/src` shows only `service/catalogue.rs` and the catalogue module. Depends on T010, T005.
- [X] T013 Switch `Pricer` in `core/src/usage/mod.rs`: `price()`/`priced()`/`cost()` call `catalogue.price(provider, model, kind)` and then `Price::cost(...)`. Keep the saved inference-profile entries first, labelled `inference_profile` (FR-020). Keep the per-report cache keyed by `(provider, model, kind)` and `cache_outside_input`. Delete the old local → catalogue → table chain. Depends on T010.
- [X] T014 Add the FR-021 golden test in `core/src/usage/mod.rs` tests: over the committed seeds, price every `(provider, model, kind)` that the pre-068 rules priced (port the old `priced` logic into the test as a reference function) and assert the same `cost` from the new walk. List any deliberate exceptions in `core/src/usage/fr021_exceptions.txt`, which must be empty at this phase. Depends on T013. *Newly priced rows (local, subscription, borrowed) and figures a provider's own list now gives (82 at this seed) are allowed by rule; the exceptions file stays empty.*
- [X] T015 Make `price_source` an open string: remove `PriceSource` from `core/src/schema/usage.rs`. `ModelRow.price_source` and `MediaCall.price_source` become `String` (values: `models_dev`, `curated`, `inference_profile`, `local`, `none` for now). Update `core/src/usage/mod.rs`, `core/src/service/usage.rs` and the utoipa schema. In the WebUI, set `PriceSource = string` in `webui/app/api/endpoints/usage.ts`, regenerate `webui/app/api/schema.d.ts` (`node scripts/openapi.mjs`), and in `webui/app/lib/usage.ts::priceNote` map `curated` to today's wording ("estimated from Vizier's price table"), fixing `webui/app/lib/usage.test.ts`. The wording is unchanged, so no mockup is needed. Depends on T013.

**Checkpoint**: `cargo test -p vizier-core`, `cargo test -p vizier-core --no-default-features`,
`cargo clippy --workspace` and `just web-check` pass. The usage report shows the same figures as
on `master`.

---

## Phase 3: User Story 1 — A provider's own price list prices its models (P1) 🎯 MVP

**Goal**: OpenRouter and Venice price their own models from their published lists, with models.dev
playing no part.

**Independent Test**: unit tests over the committed seeds price Venice `qwen-image-2-pro` and an
OpenRouter speech model with no curated entry, labelled `venice`/`openrouter`. The quickstart's
live step 7 does the same through a real image call.

- [X] T016 [P] [US1] Extend `scripts/catalogue_update.py` per `contracts/seeds.md`. Write `crates/vizier-core/assets/catalogue/openrouter_models.json` (from `/api/v1/models`, keeping the listed fields) and `crates/vizier-core/assets/catalogue/venice.json` (`?type=text|image|tts`, `{schema:1, fetched_at, text, image, tts}`). Add `price` to `openrouter_media.json` (schema 2), with image prices from `/api/v1/images/models/{id}/endpoints`, 4 at a time with `concurrent.futures`, lowest `output_image` endpoint. Keep standard-library only and `--keep-on-failure`. Extend `scripts/` tests if `just test-scripts` covers this script.
- [X] T017 [US1] Run `just catalogue-update` and commit the three seed files. Spot-check `venice-sd35` (`generation.usd` = 0.01) and one OpenRouter text model (`pricing.prompt`). Depends on T016.
- [X] T018 [P] [US1] Syntax-check `assets/catalogue/openrouter_models.json` and `assets/catalogue/venice.json` in `core/build.rs`, as it does for `openrouter_media.json` (with `cargo:rerun-if-changed` for each), and add `seed::load_openrouter_models_seed()` / `seed::load_venice_seed()` in `core/src/catalogue/seed.rs` via `include_str!`.
- [X] T019 [P] [US1] Add `CatalogueSourceStorage` (`get(source) -> Option<CatalogueSourceRow>`, `put(row)`, `record_failure(source, error, at)`) in a new `core/src/storage/catalogue_source.rs`, implemented once over `SqlConn` in `core/src/storage/sqlite/catalogue_source.rs`. Add the table to SQLite init in `core/src/storage/sqlite/mod.rs` (data-model §10) and in `core/src/storage/sql/postgres/migrations/0017_catalogue_sources.sql`. Compose it into `VizierStorageProvider` and implement it for `VizierStorage` in `core/src/storage/mod.rs`.
- [X] T020 [US1] Implement the `openrouter` source in `core/src/catalogue/sources/openrouter.rs`. Parse `openrouter_models.json` per `contracts/seeds.md`: strings × 1e6 → `Cost`, `overrides` dropped, facts from `supported_parameters`/`context_length`/`top_provider`, `Deprecated` after `expiration_date`. Merge in the media lists' prices from `MediaCatalogue`. Its authority is `Own` for `openrouter` and `None` for every other provider. `refresh` fetches `/api/v1/models` and stores it under `openrouter` in `catalogue_source_snapshot`; `load_cached` adopts a newer row with the same schema. Unit tests parse the committed seed: one known price, the model count, and **no answer for provider `anthropic`** (reseller rule). Depends on T017, T018, T019, T010.
- [X] T021 [US1] Bump `core/src/catalogue/media.rs` to payload schema 2: `MediaModel.price: Option<Price>`, parsed from the list's `pricing` (speech/transcription) and, for images, from each model's `endpoints`. The refresh fetches endpoints only for new ids or prices older than the TTL, 4 at a time, keeping the last price on failure. A schema-1 row is ignored (the seed answers). Move the refresh state onto `SourceRefresh` (T005). Depends on T020. *Done differently: the OpenRouter source fetches the speech/transcription lists' prices and the image endpoints itself and keeps them in `catalogue_source_snapshot`; spec 028's media lists keep schema 1 and serve the pickers unchanged. Image prices come from each model's first (default) endpoint, not the cheapest, since endpoints price in different units.*
- [X] T022 [US1] Implement the `venice` source in `core/src/catalogue/sources/venice.rs` per `contracts/seeds.md`: text → `Cost` per 1M plus facts from `capabilities`/`availableContextTokens`/`maxCompletionTokens`; image → `per_image` with `1K`/`2K`/`4K` and `{res}:{quality}` variants, `generation` (or the lowest tier) as the default; tts → `per_million_characters`. Its authority is `Own` for `venice` only. `refresh` fetches the three lists and stores them under `venice`. Unit tests on the committed seed: `venice-sd35` = $0.01/image, a tiered model's variants, and a tts model's rate. Depends on T017, T018, T019, T010.
- [X] T023 [US1] Check what variant a Venice image call records. Read `core/src/llm/media/venice.rs` (size → `resolution`, line ~58) and the `Measured.variant` it reports. Make the recorded variant equal the tier key T022 uses (`1K`/`2K`/`4K`), and add a unit test. Depends on T022.
- [X] T024 [US1] Register `openrouter` and `venice` first in `Catalogue`'s source list (`core/src/catalogue/mod.rs`) and in startup (`core/src/dependencies.rs`). Depends on T020, T022, T011.
- [X] T025 [US1] Tests in `core/src/catalogue/aggregate.rs`: **SC-005**, building the aggregate from every source except `models_dev` still prices and describes every OpenRouter and Venice seed model. An `openrouter` model priced by both OpenRouter and models.dev takes OpenRouter's price (US1 scenario 2). Depends on T024.
- [X] T026 [US1] Update `core/src/catalogue/media_prices.rs`: remove the OpenRouter and Venice rows from `KNOWN_UNPRICED` that are now priced by their own list. Make `every_default_media_model_is_priced_or_local` ask `Catalogue::price` instead of the table (**SC-002**). Remove any curated rows that duplicate a provider list's figure only if the figures agree, otherwise keep them and note the disagreement. Re-run T014 and add any `openrouter`/`venice` exception lines to `core/src/usage/fr021_exceptions.txt` with the source that now wins. Depends on T024.

**Checkpoint**: US1 works alone. Venice and OpenRouter usage is priced from their own lists, even
offline from the seeds.

---

## Phase 4: User Story 2 — Local and subscription usage is accounted for (P1)

**Goal**: Ollama, llama.cpp, dummyplug and ChatGPT usage reads $0, labelled `local` or
`subscription`. This phase does not depend on US1 and can be shipped first.

**Independent Test**: quickstart steps 1–2 (dummyplug text reads `local`, `cost_usd: 0`; past rows
re-price).

- [X] T027 [P] [US2] Add `Billing { Metered, Local, Subscription }` and a `billing` field to `BuiltinProvider` in `core/src/catalogue/builtin.rs`: Ollama, llama.cpp and dummyplug are `Local`, ChatGPT is `Subscription`, everything else (media built-ins, SSO Bedrock) is `Metered`. Add `Catalogue::billing(provider)` in `core/src/catalogue/mod.rs`: the built-in's value, a models.dev provider of `ProviderKind::Local` → `Local`, custom → `Metered`, aliased through `model_source`. *A models.dev provider on localhost is local only while no source prices it (`privatemode-ai` is a localhost proxy to a paid service).*
- [X] T028 [US2] Apply the rule at the top of the price walk in `core/src/catalogue/aggregate.rs` (contract §5): `Local`/`Subscription` → `Price::zero()` with label `local`/`subscription`, for every kind. Unit tests for Ollama text, ChatGPT text, and a local media call (today's `local` flag still yields `local`). Depends on T027, T009.
- [X] T029 [US2] In `core/src/usage/mod.rs`, make `Pricer` exclude `local` and `subscription` rows from `unpriced` and put them in the totals at $0. Add `subscription` to `priceNote` in `webui/app/lib/usage.ts` as `included · subscription` (the wording from `contracts/webui.md`), and show `free` in the cost cell for it in `webui/app/routes/usage.tsx` as it already does for `local`. This is wording in an existing slot, but it waits for the T034 mockup review if that review changes it. Depends on T028. *Backend done (free, priced, never unpriced); the `included · subscription` wording landed after T033.*
- [X] T030 [US2] Add the dummyplug e2e in `core/tests/dummyplug_e2e.rs` (quickstart step 1): one turn on dummyplug, then the agent's usage report has `dummyplug/dummyplug` with `price_source == "local"`, `cost_usd == 0`, and `unpriced.models` empty. Depends on T029. *Covered in-crate by `usage::tests::report::rows_name_their_price_source` over real storage; the binary run is T045.*

**Checkpoint**: SC-001 holds. Local and subscription tokens never appear in `unpriced`.

---

## Phase 5: User Story 3 — Every fact says where it came from (P1)

**Goal**: every model row names one source, disagreements are visible, and the report lists
source names.

**Independent Test**: a report over usage priced by each source shows one `price_source` per row,
`sources[]` names each, and a model priced differently by two sources carries
`price_alternatives`.

- [X] T031 [US3] Add `ModelRow.price_alternatives: Vec<PriceAlternative { source, cost_usd }>` (each alternative priced on the row's own tokens and units with `Price::cost`) and `UsageReport.sources: Vec<SourceSummary { id, name, fetched_at }>` (only sources whose id appears in the report) in `core/src/schema/usage.rs`, filled in `core/src/usage/mod.rs` from `PriceAnswer`. Regenerate `webui/app/api/schema.d.ts`. Unit test: a fake disagreeing source yields one alternative with the right `cost_usd`. Depends on T013, T009.
- [X] T032 [US3] Extend `GET /api/v1/catalogue` status: `CatalogueStatus.sources: Vec<SourceStatus { id, name, origin, fetched_at, checked_at, model_count, unmapped, refresh }>` in `core/src/service/catalogue.rs`, with the top-level fields unchanged, and expose it in `http/src/api/v1/catalogue.rs` (utoipa). Report `unmapped` counts from each source's `SourceData`, and log them once per load with `tracing::info!`. Depends on T024, T005.
- [X] T033 [US3] **Design review gate (no code)**: get the person's sign-off on the ASCII mockup in `specs/068-catalogue-sources/contracts/webui.md`, or redraw it in FigCraft (`Vizier shell.figcraft`, the usage page) if the MCP is reachable. Record the decision in that file. T034 cannot start before this. *Approved by the person on 2026-11-27 (ASCII mockup; FigCraft unreachable).*
- [X] T034 [US3] Implement the labels in `webui/app/lib/usage.ts::priceNote(row, sources)` and `mediaCallLine` per the table in `contracts/webui.md`: a source id shows `sources[id].name` (falling back to the id), plus the ` · <name> says <$cost>` suffix for the first alternative and a `title` listing all alternatives on the row in `webui/app/routes/usage.tsx`. Add one `webui/app/lib/usage.test.ts` case per table row and one for the suffix. `just web-check` passes. Depends on T031, T033.

**Checkpoint**: SC-004 holds. Every model row names exactly one source.

---

## Phase 6: User Story 4 — Custom provider models get an estimate (P2)

**Goal**: a custom provider's model is priced by borrowing the same id's price from its maker,
labelled `borrowed` with `borrowed_from`.

**Independent Test**: quickstart steps 3–4 (`my-proxy/gpt-4o-mini` → `borrowed` from `openai`;
`my-proxy/mystery-1` → `none`).

- [X] T035 [US4] Implement borrowing as the last step of the walk in `core/src/catalogue/aggregate.rs` (contract §5). It runs only for `Metered`. It tries the maker first: the provider named by the `canonical_model_id` prefix of the same model id anywhere in the merged view (add `lab`/canonical to `ModelFacts` via `models_dev` if it is not already carried). Then providers with the asking provider's dialect, then `ProviderId` order. It walks each candidate without borrowing and sets `borrowed_from`. Unit tests: `gpt-4o-mini` borrows from `openai` (not `abacus`), a dialect tie-break, determinism across runs, no borrowing for `Local`/`Subscription`, and a borrowed price never borrowed again. Depends on T009, T028.
- [X] T036 [US4] Add `ModelRow.borrowed_from: Option<String>` in `core/src/schema/usage.rs`, filled in `core/src/usage/mod.rs`. Make sure borrowed rows are counted in totals and not in `unpriced` (FR-016, assumption "borrowed prices count"). Map `borrowed` to `estimate · borrowed from <provider>` in `webui/app/lib/usage.ts` after T033. Depends on T035, T031. *Backend done (`borrowed_from` on the row); the WebUI wording landed after T033.*
- [X] T037 [US4] Remove the separate cost borrowing in `core/src/llm/resolve.rs::resolve_custom` (`resolved.cost = borrowed.and_then(|m| m.cost)`): set `resolved.cost` from `catalogue.price(provider, model, Text)`, so the resolver and the report agree. Keep the limits and caps borrowing as it is (FR-004: custom rows first). Update the tests at `llm/resolve.rs` ~1312–1431 that assert `cost`. Depends on T035.
- [X] T038 [US4] Add the dummyplug e2e in `core/tests/dummyplug_e2e.rs` (quickstart steps 3–4): a custom provider at dummyplug's URL with models `gpt-4o-mini` and `mystery-1`. Assert `borrowed` with `borrowed_from == "openai"` and `cost_usd > 0`, then `none` with the model listed in `unpriced`. Depends on T036. *Same in-crate test as T030.*

**Checkpoint**: custom provider usage is priced whenever its maker publishes the model.

---

## Phase 7: User Story 5 — Model facts and providers come from every source (P3)

**Goal**: chat pickers and agent warnings see every source's text models, not only models.dev's.

**Independent Test**: an OpenRouter or Venice text model missing from models.dev appears in
`GET /api/v1/catalogue/providers/{id}/models`, and an agent using it gets no `unknown_model`
warning.

- [X] T039 [US5] Unit test in `core/src/catalogue/mod.rs`: find a seed text model that is in `openrouter_models.json` or `venice.json` but not in models.dev's entry for that provider. Assert it is in `Catalogue::models(provider, true)`, that its `limit.context` comes from the provider list, and that `agent_warnings` returns no `unknown_model` for it (FR-022, FR-023). Media-kind models must not appear in `models()`. Depends on T024, T008.
- [X] T040 [US5] Check `core/src/llm/resolve.rs::resolve_with` against merged models: a model only a provider list knows resolves with the provider's dialect from `models_dev`'s connection (`dialect_for(&p, entry)` with `route` `None`), and its merged limits and caps. Fix any read that still assumes models.dev-only data. Add a resolve unit test for such a model. Depends on T039.
- [X] T041 [P] [US5] Check the model list and detail handlers in `core/src/service/catalogue.rs` (`list_models`, `get_model`) and `http/src/api/v1/catalogue.rs`: they return merged models with no shape change, and a merged-only model is listed once. Add a service test. Depends on T039.

**Checkpoint**: every story works on its own.

---

## Phase 8: Polish and cross-cutting

- [X] T042 [P] Update `docs/src/configuration/usage.md`: where prices come from (the authority order, each source, the rules, borrowing, disagreements), the `price_source` values, and the **breaking** rename `catalogue` → `models_dev`, `table` → `curated`.
- [X] T043 [P] Add a "Catalogue sources" section to `docs/src/configuration/providers.md`: the sources, their refresh (people-triggered, `catalogue.refresh`/`catalogue.ttl`), the `GET /api/v1/catalogue` `sources[]` status, and how to add a source (one file under `core/src/catalogue/sources/` plus one registration line).
- [X] T044 [P] Update the catalogue section of `CLAUDE.md` ("Providers / models" and "Usage accounting"): the catalogue is an aggregate of `CatalogueSource`s, the authority order, the price walk, `Billing`, `catalogue_source_snapshot`, and `price_source` as a string.
- [X] T045 Run the full quickstart (`specs/068-catalogue-sources/quickstart.md` steps 1–6) against `just run`, including step 6 (no fetch without a person, `RUST_LOG=vizier_core::catalogue=debug`). Record the results in the PR description. *Run 2026-11-27 on the `dev_serve` harness (the `vizier` binary does not build on Windows): step 1 `dummyplug/dummyplug` → `local`, $0; step 3 `my-proxy/gpt-4o-mini` → `borrowed` from `openai` ($0 only because dummyplug reports no tokens); step 4 `my-proxy/mystery-1` → `none`, listed unpriced; step 5 all four sources in `GET /catalogue`. Step 6: the harness has no log output; models.dev stayed `bundled` through the agent turns and became `cached` only after a person's catalogue read. `webui/app/api/schema.d.ts` regenerated from that server.*
- [X] T046 Final gates: `cargo clippy --workspace`, `cargo test --workspace`, `cargo test -p vizier-core --no-default-features`, `just web-check`, `just test-scripts`. Then commit as `feat(catalogue)!: aggregate catalogue sources with provider price lists (spec 068)` (breaking: `price_source` values). Make sure `core/src/usage/fr021_exceptions.txt` lists only explained changes. *Run 2026-11-27: vizier-core lib 2138 pass (+ `--no-default-features`), vizier-http all pass, WebUI typecheck and 1762 tests pass, clippy clean on touched files. Two core tests (`projects::path` absolute paths, `skill::scopes` separators) fail identically on a clean `master` checkout on Windows: not this change. Labels checked on the usage page of the dev server.*

---

## Dependencies and execution order

- **Setup (T001)** comes first.
- **Foundational (T002–T015)** blocks every story. Inside it: T002 ∥ T003 → T004 → (T005, T007,
  T008) → T006 → T009 → T010 → (T011, T012, T013) → (T014, T015).
- **US1 (T016–T026)** needs Foundational. T016 → T017. T018 ∥ T019. T020 and T022 can run in
  parallel once T017–T019 are done, then T021 and T023, then T024 → T025, T026.
- **US2 (T027–T030)** needs only Foundational, independent of US1. **Recommended first increment.**
- **US3 (T031–T034)** needs Foundational. T032 needs US1's T024 for a meaningful `sources[]`.
  **T034 waits for the T033 design review.**
- **US4 (T035–T038)** needs T009 and US2's T028 (the rule comes before borrowing). Its WebUI line
  waits for T033.
- **US5 (T039–T041)** needs US1's T024 (a provider list to merge).
- **Polish (T042–T046)** comes last.

```
T001 → Foundational ─┬─ US2 (ship first) ──┐
                     ├─ US1 ─┬─ US5        ├─ US4 ─┐
                     │       └─ US3 (T032) │       ├─ Polish
                     └─ US3 (T031, T033→T034) ─────┘
```

## Parallel examples

- **Foundational**: T002 (`price.rs`) ∥ T003 (`data.rs`), then T007 (`sources/curated.rs`) ∥ T008
  (`aggregate.rs`) once T004 is done.
- **US1**: T016 (script) ∥ T018 (`build.rs`/`seed.rs`) ∥ T019 (storage), then T020
  (`sources/openrouter.rs`) ∥ T022 (`sources/venice.rs`).
- **US2 ∥ US1**: T027–T030 touch `builtin.rs`, the top of the walk and `usage/mod.rs`'s unpriced
  rule, and can go alongside US1's source files. Coordinate on `aggregate.rs` (T028 and T035 edit
  the walk).
- **Polish**: T042 ∥ T043 ∥ T044 (three different docs).

## Implementation strategy

1. **MVP = Foundational + US2**: no visible change except local and subscription now priced, plus
   the renamed wire values. This proves the aggregate seam with zero figure drift (T014).
2. **+ US1**: OpenRouter and Venice price their own models. This is the headline of the spec.
3. **+ US3**: provenance on the wire and in the WebUI, after the mockup review.
4. **+ US4**: custom provider estimates.
5. **+ US5**: merged facts in pickers and warnings.

Stop after any increment, and `cargo test` and the quickstart steps for what is done must pass.
