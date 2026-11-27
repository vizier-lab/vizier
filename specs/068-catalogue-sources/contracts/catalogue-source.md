# Contract: `CatalogueSource` and the aggregate

The engine-internal seam (`crates/vizier-core/src/catalogue/source.rs`). A new source is written
from this page alone.

## 1. The trait

```rust
pub trait CatalogueSource: Send + Sync {
    /// Stable id, also the wire label (`openrouter`). Never changes once shipped.
    fn id(&self) -> SourceId;
    /// Display name ("OpenRouter").
    fn name(&self) -> &'static str;
    /// What this source is for `provider` (data-model §2). Pure; no I/O.
    fn authority(&self, provider: &ProviderId) -> Authority;
    /// The current normalized snapshot (seed until a refresh). Cheap: clones an `Arc`.
    fn data(&self) -> Arc<SourceData>;
    /// The price this source gives `(provider, model, kind)`, or `None`.
    /// Default: `data().providers[provider].models[model].price` when its kind matches `kind`
    /// (or the model's kind is unknown). `curated` overrides it for prefix ids.
    fn price(&self, provider: &ProviderId, model: &str, kind: UsageKind) -> Option<Price>;
    /// Fetch upstream, normalize, store, and swap. Only called by `SourceRefresh`.
    fn refresh<'a>(&'a self, ctx: &'a RefreshCtx) -> BoxFuture<'a, crate::Result<()>>;
    /// Startup: adopt a stored snapshot newer than the seed. Never touches the network.
    fn load_cached<'a>(&'a self, storage: &'a VizierStorage) -> BoxFuture<'a, ()>;
    fn refresh_state(&self) -> &SourceRefresh;
}
```

A source with nothing to refresh (`curated`) returns `Ok(())` from `refresh` and has no stored
row.

## 2. Registration and order

`Catalogue::new` takes `Vec<Arc<dyn CatalogueSource>>` in registration order:
`openrouter`, `venice`, `curated`, `models_dev`. Authority decides precedence, and registration
order only breaks ties within one level. This list is **the one place** the order is written
(FR-004, SC-006). No other code names a source id, except each source's own file and the WebUI's
optional icon map.

## 3. Mapping rules (FR-003)

- A source maps every upstream entry to `(ProviderId, model id)` before returning data. An entry
  that does not map is counted in `SourceData.unmapped` and dropped. It is never guessed.
- `openrouter`: every entry → provider `openrouter`, model id unchanged.
- `venice`: every entry → provider `venice`, model id unchanged.
- `models_dev`: provider keys as today (legacy names through `ProviderId` parsing).
- `curated`: the `provider` field must parse as a `ProviderId`. A failure is skipped with a warning
  (today's behaviour).

## 4. Merge (facts)

Run on every `swap` from any source, and once at startup after `load_cached`:

1. Start from `models_dev`'s connections for the provider list (built-ins and custom are added at
   read time, as today).
2. For each provider, take the union of model ids over sources whose `authority(provider) !=
   None`.
3. For each model and each field in data-model §5, take the value from the most authoritative
   source that has it. `caps` is merged per sub-field (`tool_call`, `reasoning`, `input`, …).
4. Record the winning `SourceId` per field in `provenance`.
5. Swap in the new `Arc<Merged>`. Readers holding the old `Arc` are unaffected.

A source whose data failed to parse contributes nothing to that merge. The merge itself cannot
fail.

## 5. Price walk (`Catalogue::price`)

```
provider := builtin::model_source(provider) or provider
match billing(provider):
  Local        → { price: Some(zero), source: "local" }
  Subscription → { price: Some(zero), source: "subscription" }
  Metered      → continue
for level in [Own, Curated, General]:
  for s in sources where s.authority(provider) == level (registration order):
    if let Some(p) = s.price(provider, model, kind):
      if winner is None: winner = (s.id, p)
      else if p != winner.price and alternatives.len() < 3: alternatives.push((s.id, p))
if winner: return it with alternatives
borrow: for other in merged providers ordered by
          (the model's maker first — the provider named by the canonical_model_id prefix of the
           same id anywhere in the merged view —, then same dialect as provider, then ProviderId):
          if other != provider and billing(other) == Metered:
            walk levels for (other, model, kind) without borrowing
            first hit → { source: "borrowed", borrowed_from: other }
none   → { price: None, source: "none" }
```

`zero` is a `Price` whose `cost` is `0.0` for every kind. Borrowing never recurses and never
borrows a borrowed price.

## 6. Refresh

`SourceRefresh` holds the policy once (TTL from `catalogue.ttl`, a 1 h backoff after a failure,
one in flight per source). `catalogue::maybe_refresh(deps)` calls it for every source, and
`catalogue::refresh_now(deps)` waits up to 30 s for all of them and reports per-source errors. The
only callers are the existing people-only handlers (`/catalogue/*` reads, `GET /providers`,
`GET /providers/custom`, `POST /catalogue/refresh`, `POST /catalogue/media-models/{capability}/refresh`).
Agent, scheduler and dream code never call them (FR-011).

## 7. Failure

- A seed that does not parse: that source starts empty with a `warn!`, and the others answer.
- A refresh that fails: `last_error`/`failed_at` are stored and the last good data stays.
- A stored row with another `schema`: ignored, and the seed answers.
