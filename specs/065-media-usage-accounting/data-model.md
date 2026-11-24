# Data Model: Usage and Spend of Images, Audio and Embeddings

Extends `specs/018-usage-accounting/data-model.md`. Nothing there is removed or renamed.

## 1. `UsageKind` (new, `schema/usage.rs`)

| Value | `as_str` | Label | Unit |
|---|---|---|---|
| `Text` | `text` | Text | tokens |
| `Image` | `image` | Images | images |
| `Speech` | `speech` | Speech | characters (and audio seconds out) |
| `Transcription` | `transcription` | Transcription | audio seconds in |
| `Embedding` | `embedding` | Embeddings | input tokens |

`UsageKind::ALL` is in this order and append-only. Every completion row is `Text`.

## 2. `UsageActivity` (extended)

`Indexing` (`indexing`, label "Indexing") is appended to `ALL`. It is set only on media records with
no conversation (R2). `missing_before_accounting()` is unchanged; media's own start is
`usage.media_accounting_since` (§6).

## 3. `UsageEvent` / `usage_event` (extended)

New fields and columns (all others as 018):

| Field | Column | Type | Rule |
|---|---|---|---|
| `kind` | `kind` | `UsageKind` / `TEXT NOT NULL DEFAULT 'text'` | |
| `variant` | `variant` | `Option<String>` / `TEXT` | Image only: `"<size>"` or `"<size>/<quality>"`, e.g. `1024x1024/high` |
| `units.images` | `images` | `u64` / `INTEGER NOT NULL DEFAULT 0` | Image |
| `units.characters` | `characters` | `u64` | Speech |
| `units.audio_ms` | `audio_ms` | `u64` | Speech (out), Transcription (in); 0 = unknown |
| `measured` | `measured` | `bool` | true when any amount was measured, not reported (R4) |
| `local` | `local` | `bool` | the model runs on this machine (fastembed, Whisper, Kokoro, Ollama, llama.cpp, dummyplug) |
| `call_id` | `call_id` | `Option<String>` / `TEXT` | the `execute_python` call that made it (R10) |

For media rows: `tokens` holds reported (or, for embeddings, measured) tokens; `turn_id`/`round`
come from the scope; `served_model` from `Reported.served_model`; `legacy` is always false.
`provider` is the `ProviderId` the media setting resolves to, or the local engine's name
(`local`, `whisper`, `kokoro`, `dummyplug`) for a provider with no credentials.

Validation: a record is written only for an answered request (FR-005). Units are never negative;
`images ≥ 1` for an Image record.

Indexes: existing three, plus `usage_event_call (call_id) WHERE call_id IS NOT NULL`.

Lifecycle: as 018 — deleted with the agent, kept with the conversation.

## 4. `Reported` (new, `llm/media/mod.rs`)

What a provider client found in the response; every field optional.

```text
Reported { input_tokens, output_tokens, images, characters, audio_ms, served_model }
```

Return types of the provider traits wrap their output with it (research R1).

## 5. `UsageScope` (extended)

- `UsagePurpose::Media` — activity is the session's `usage_activity()`, or `Indexing` with no
  session.
- `call_id: Option<String>`.
- `MEDIA_SCOPE` task-local: the ambient scope the embedder reads (R2).
- `ToolContext::usage_scope()` — the scope a tool's media calls use.

## 6. State keys

| Key | Value | Set |
|---|---|---|
| `usage.accounting_since` | RFC 3339 | 018, unchanged |
| `usage.media_accounting_since` | RFC 3339 | once, the first startup with this feature |

## 7. `MediaPrices` (new, `catalogue/media_prices.rs`)

Parsed once from the embedded `assets/media-prices.json` ([contracts/media-prices.md](contracts/media-prices.md)).

```text
MediaPrices { as_of, entries: Vec<MediaPrice> }
MediaPrice  { provider: ProviderId, model: ModelPattern, kind: UsageKind,
              tokens: Option<Cost>,                // USD / 1M, the catalogue's `Cost` type
              per_image: Option<ImagePrice>,       // { default: f64, variants: {variant → f64} }
              per_million_characters: Option<f64>,
              per_audio_minute: Option<f64>,
              source: Url, as_of: Date }
ModelPattern = Exact(String) | Prefix(String)      // "gpt-image-1" | "eleven_*"
```

Lookup: exact before prefix, longest prefix wins; `provider` aliases resolve as `ProviderId` does.

## 8. `PriceSource` (new)

`Catalogue | Table | Local | None` — which rule of research R6 priced a figure.

## 9. Report shapes (extended, `schema/usage.rs`)

- `Units { images, characters, audio_seconds, tokens }` — a kind's amount, only its own fields set.
- `BreakdownRow` + `units: Option<Units>`.
- `KindRow { row: BreakdownRow, kind: UsageKind }`; `UsageReport.by_kind: Vec<KindRow>`.
- `ModelRow` + `kind: UsageKind`, `price_source: PriceSource`; `cache_hit_rate` stays `null` for
  non-text rows.
- `UsageSummary` + `kind_share_of_all: Option<f64>` (set when filtered to a kind).
- `DailyUsage` + `by_kind: BTreeMap<UsageKind, DayActivity>`.
- `UnpricedNote` + `units: BTreeMap<UsageKind, Units>` beside `tokens`.
- `UsageReport` + `kind: Option<UsageKind>`, `media_accounting_since: Option<DateTime<Local>>`.
- `ConversationCall` (018's per-call row) + `kind`, `units`, `variant`.
- `MediaCall { call_id, kind, provider, model, variant, units, measured, local, cost_usd,
  price_source, at }` — the `/usage/calls` row.

## 10. Relationships

```text
Agent 1 ── * UsageEvent (kind, activity) ── 0..1 Conversation (channel, topic)
UsageEvent 0..1 ── call_id ── history ToolCall { call_id }
UsageEvent (provider, model, variant) ── priced by ── Catalogue | MediaPrices
```
