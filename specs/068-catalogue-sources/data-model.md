# Data Model: Catalogue Sources (spec 068)

Rust names are given where they are decided. Everything here is in memory unless marked
**stored**.

## 1. `SourceId`

A stable, lowercase id, also the wire label: `models_dev`, `openrouter`, `venice`, `curated`.
Rule and fallback labels share the namespace but are not sources: `local`, `subscription`,
`inference_profile`, `borrowed`, `none`. Each source also has a display `name` ("models.dev",
"OpenRouter", "Venice", "vizier's price table").

## 2. `Authority`

What a source is for a given provider, in descending order:

| Authority | Meaning | Sources |
|---|---|---|
| `Own` | the provider's own published list | `openrouter` for `openrouter`, `venice` for `venice` |
| `Curated` | vizier's hand-checked figures | `curated` for every provider |
| `General` | a catalogue of many providers | `models_dev` for every provider |
| `None` | the source says nothing about this provider | `openrouter`/`venice` for any other provider |

Ties within one level (several `General` sources later) follow registration order.

## 3. `Billing` (on `BuiltinProvider`, and derived for catalogue providers)

`Metered` (default) · `Local` (Ollama, llama.cpp, dummyplug, and a models.dev provider whose
`kind` is `Local` **while no source prices any of its models**: `privatemode-ai` is a localhost
proxy to a paid service, found by the FR-021 golden test) · `Subscription` (ChatGPT). Asked before any source. `Local` and
`Subscription` price every kind at $0 with that label and are never borrowed for.

## 4. `SourceData`

One source's normalized snapshot, already mapped onto vizier's ids (FR-003):

| Field | Type | Notes |
|---|---|---|
| `fetched_at` | `Timestamp` | when the upstream data was fetched (seed: build time) |
| `origin` | `Bundled \| Cached` | as `CatalogueSource` today (renamed `SnapshotOrigin`, since "source" now means something else) |
| `providers` | `BTreeMap<ProviderId, SourceProvider>` | |
| `unmapped` | `u32` | entries dropped because their names did not map (logged once per load) |

`SourceProvider`: `connection: Option<ProviderConnection>` (only `models_dev` fills it: `name`,
`env`, `api`, `npm`, `doc`, derived `dialect`/`kind`, FR-002) and
`models: BTreeMap<String, ModelFacts>`.

## 5. `ModelFacts`

Every field is optional, and absent means "this source does not say".

| Field | Type | Notes |
|---|---|---|
| `name` | `Option<String>` | |
| `kind` | `Option<ModelKind>` | `Text \| Image \| Speech \| Transcription \| Embedding`, from output modalities |
| `caps` | `Capabilities` | existing type; each field already optional |
| `limit` | `Limits` | existing type |
| `price` | `Option<Price>` | §6 |
| `status` | `Option<ModelStatus>` | |
| `release_date` | `Option<NaiveDate>` | |
| `route` | `Option<RouteOverride>` | `models_dev` only (connection data) |
| `family`, `knowledge`, `lab` | `Option<String>` | `models_dev` only |

## 6. `Price`

| Field | Type | Notes |
|---|---|---|
| `tokens` | `Option<Cost>` | existing `Cost`, USD per 1M tokens (OpenRouter strings × 1e6) |
| `per_image` | `Option<ImagePrice>` | existing type: `default` + `variants` (Venice `1K`/`2K`/`4K`, `{res}:{quality}`) |
| `per_million_characters` | `Option<f64>` | speech |
| `per_audio_minute` | `Option<f64>` | transcription (`per_audio_hour` is folded in at parse time) |

`Price::cost(&self, kind, tokens, units, variant, cache_outside_input) -> Option<f64>` is the
**one** pricing function. It wraps today's `estimate_cost` and `MediaPrice::unit_cost`, in
today's order: tokens when the record has tokens and the price has token rates, else units. Prices
are equal (for disagreements) when every present rate matches within 1e-9.

## 7. The merged view

`Catalogue` holds `Arc<Merged>`, rebuilt whenever a source swaps its data:

- `data: CatalogueData`: the **existing** type, filled from the merge. Providers come from
  `models_dev` connections plus built-ins and custom providers, as today. Each provider's `models`
  is the union of every source's models for it (FR-022), each field taken from the most
  authoritative source that has it (FR-005). Readers (`provider`, `model`, `models`,
  `find_model_by_id`) keep reading `CatalogueData`. `models()` adds `kind` ∈ {`Text`, unknown}.
- `provenance: HashMap<(ProviderId, String), FactSources>`: which `SourceId` gave `name`,
  `limit`, `caps` and `kind` (for status and tests; no new API needs it yet).

A custom provider's row facts are applied where they are today (`resolve_custom`, `agent_warnings`),
ahead of the merged view (FR-004).

## 8. `PriceAnswer` (from `Catalogue::price(provider, model, kind)`)

| Field | Type | Notes |
|---|---|---|
| `price` | `Option<Price>` | `None` = unpriced |
| `source` | `SourceId` or rule label | `local`, `subscription`, a source id, `borrowed`, `none` |
| `borrowed_from` | `Option<ProviderId>` | set only when `source == borrowed` |
| `alternatives` | `Vec<(SourceId, Price)>` | up to 3 later sources whose price differs (FR-006) |

The walk: billing rule → sources with `Own`, then `Curated`, then `General`, asking each
`source.price(provider, model, kind)`. The first `Some` wins, and later differing ones become
alternatives. If none answers and billing is `Metered`: borrow (same model id at another provider:
its maker first, named by `canonical_model_id`, then the same dialect, then `ProviderId` order,
a non-borrowed price of the same kind). Otherwise
`none`. Provider aliases go through `builtin::model_source` first, as today.

`Pricer` keeps inference-profile entries ahead of this call (FR-020), labelled
`inference_profile`.

## 9. `SourceRefresh`

One per source: `in_progress: AtomicBool`, `failure: Mutex<Option<(String, Timestamp)>>`,
`checked_at`. It replaces the duplicated fields in `refresh::RefreshState` and
`media::MediaCatalogue`. Policy: `catalogue.refresh`, `catalogue.ttl`, a 1 h failure backoff,
started only by `maybe_refresh` / `refresh_now`, the existing people-only call sites. Logo
fetching stays with `models_dev`.

## 10. Stored

**`catalogue_source_snapshot`** (new; SQLite init + PostgreSQL `0017`):

| Column | Type | Notes |
|---|---|---|
| `source` | TEXT PK | `openrouter`, `venice` (models.dev keeps `catalogue_snapshot`) |
| `schema` | INTEGER | payload version; another value is ignored and the seed answers |
| `fetched_at`, `checked_at` | INTEGER ms | |
| `last_error` | TEXT NULL | |
| `failed_at` | INTEGER ms NULL | |
| `data` | BLOB | the trimmed upstream JSON |

**`media_catalogue_snapshot`**: unchanged (as built). The OpenRouter source fetches the speech and
transcription lists' prices and each image model's endpoint pricing itself and keeps them in its
`catalogue_source_snapshot` row, so spec 028's media lists keep schema 1 and only serve pickers.

## 11. Report wire types (`schema/usage.rs`)

- `ModelRow.price_source` and `MediaCall.price_source`: `String` (a `SourceId` or rule label)
  instead of the `PriceSource` enum. The enum is removed.
- `ModelRow.borrowed_from: Option<String>`, `ModelRow.price_alternatives:
  Vec<{source, cost_usd}>`, priced on that row's own tokens and units.
- `UsageReport.sources: Vec<{id, name, fetched_at}>`, the display names for every label used.
