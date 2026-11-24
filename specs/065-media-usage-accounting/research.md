# Research: Usage and Spend of Images, Audio and Embeddings

Phase 0 of [plan.md](plan.md). Every decision below resolves an unknown in the plan's Technical
Context. Spec 018's research (`specs/018-usage-accounting/research.md`) is the baseline; this
feature extends it and changes none of its decisions for completions.

## R1. Where a media call is recorded

**Decision**: The four media handles — `VizierImageGen`, `VizierTts`, `VizierStt`,
`VizierEmbedder` — are the record point, exactly as `VizierModel::complete_until` is for
completions. Each handle gains the agent id, its `(provider, model)`, a `local` flag and the
storage handle when it is built, and records one `UsageEvent` per answered request through one
shared helper, `usage::record::spawn_record(storage, event)` (moved out of `VizierModel::record`,
which then calls it too). The write is spawned and a failure is only logged (FR-007).

The provider traits (`VizierImageGenModel`, `VizierTtsModel`, `VizierSttModel`,
`VizierEmbeddingModel`) change their return types to carry what the provider reported:

| Trait | Today | After |
|---|---|---|
| `VizierImageGenModel::generate` | `(Vec<u8>, String)` | `GeneratedImage { bytes, mime, reported: Reported }` |
| `VizierTtsModel::generate_speech` | `Vec<u8>` | `Speech { audio, reported: Reported }` |
| `VizierSttModel::transcribe` | `String` | `Transcript { text, reported: Reported }` |
| `VizierEmbeddingModel::embed_texts` | `Vec<Vec<f64>>` | `Embeddings { vectors, reported: Reported }` (`embed_text` stays a default over it) |

`Reported` is a small struct of optional figures (`input_tokens`, `output_tokens`, `images`,
`characters`, `audio_ms`, `served_model`) a client fills from the response when the provider
sends them, `Reported::default()` otherwise. The handle measures whatever is missing (R4).

**Rationale**: One seam per kind, at the one type every caller already goes through, means no call
site can make an unrecorded media call — the same guarantee 018 R1 bought for completions. The
handles already hold the provider choice; the `match` over providers stays in the four
constructors, where it is today (Principle II).

**Alternatives considered**: Recording in each tool (`image_gen`, `tts`, `stt`) — misses the
spoken-reply path in `agents/agent/mod.rs`, live voice and every embedding. Recording in
`media::endpoint`'s HTTP layer — sees bytes, not meaning (an image count, a batch of passages), and
never sees local models.

## R2. What the call is for: scope

**Decision**: `generate`, `generate_speech` and `transcribe` on the handles take a required
`&UsageScope`, like `complete`. `UsageScope` gains `UsagePurpose::Media` (activity = the session's
`usage_activity()`, or **Indexing** with no session) and an optional `call_id`. Call sites:

| Call site | Scope |
|---|---|
| `image_gen`, `tts`, `stt` tools | `ctx.usage_scope()` — the turn's session, turn id, round and the `execute_python` call id, carried on `ToolContext` (new field, set where `prompt_with` builds each round's context) |
| Spoken reply (`agents/agent/mod.rs`, TTS) and voice-message transcription (STT in `chat`) | the turn's scope, no call id |
| Live voice (`live_voice/engines.rs`) | `UsageScope::media(Some(session))` for the call's conversation (FR-008) |

Embeddings are reached through the indexer, from `BundleMemoryStore` (writes, imports, rollbacks,
searches), the skill index and the per-agent conversion in `AgentHost::start`, with many callers
and no session on the path. Threading a scope through `MemoryStorage` for accounting alone is out of
proportion, so the embedder reads an **ambient** scope: a `tokio::task_local!` `MEDIA_SCOPE` that
`RouterBridge` sets around each tool call it dispatches (from that call's `ToolContext`), that
`prompt_with` sets around the turn's automatic recall, and that the dream cycle sets around its
stages. With no ambient scope (startup conversion, re-index after a model change, a person's memory
edit over HTTP) the record has no conversation and activity **Indexing** (FR-003).

**Rationale**: Explicit where the session is at hand (the compiler finds every site, 018 R1);
ambient only for the one kind whose path has no session, and wrapped around futures, so it works
across `spawn_blocking` hops in the sandbox bridge.

**Alternatives considered**: A session parameter on `DocumentIndexer`/`MemoryStorage` — touches
every memory caller, the trait, both backends and the tests for one field. Attributing every
embedding to Indexing — contradicts the spec's edge case for search embeddings.

## R3. What providers report

**Decision**: Parse the figures below where the response carries them; everything else is measured
(R4) and marked `measured`. A client that cannot find a figure never fails the call.

| Kind | Provider | Reported |
|---|---|---|
| Embedding | OpenAI, Mistral, Together, OpenRouter, Copilot (OpenAI-shaped) | `usage.prompt_tokens` |
| Embedding | Voyage | `usage.total_tokens` |
| Embedding | Cohere | `meta.billed_units.input_tokens` |
| Embedding | Bedrock Titan / Cohere | `inputTextTokenCount` / none |
| Embedding | Ollama | `prompt_eval_count` (local) |
| Embedding | Gemini | none |
| Image | OpenAI `gpt-image-*` | `usage.input_tokens`, `usage.output_tokens` |
| Image | others | none (one image per call) |
| Transcription | OpenAI `gpt-4o-*-transcribe` | `usage.input_tokens`, `usage.output_tokens` (type `tokens`); `whisper-1`: `usage.seconds` (type `duration`) |
| Transcription | Groq | `x_groq.usage` / `duration` where present |
| Transcription | others | none |
| Speech | ElevenLabs | `character-cost` response header |
| Speech | others | none |

**Rationale**: Reported figures are what the provider bills; measuring is the fallback the spec
requires (FR-004). The table is the parsing work list for implementation; a provider not listed is
measured.

## R4. Measuring what is not reported

**Decision**:

- **Image**: `images = 1` per `generate` call (every client returns one image); `variant` = the
  requested size (or the provider's default size) and quality when the client sets one.
- **Speech**: `characters = text.chars().count()` of the text sent; `audio_ms` from the returned
  audio when it is WAV/PCM (header or the Gemini 24 kHz rate), left unset for compressed formats.
- **Transcription**: `audio_ms` from the input when it is WAV (live voice utterances and most
  voice messages after conversion); otherwise from the provider's duration when reported; otherwise
  unset, and the call is still counted.
- **Embedding**: `input_tokens = Σ ceil(bytes / 4)` over the texts, the same estimate the prune rule
  uses (spec 060), marked measured.

**Rationale**: Cheap, deterministic, no new dependency (no audio decoder) — Principle I. An unset
amount still counts as a call and is shown as such.

## R5. The shipped media price table

**Decision**: One data file, `crates/vizier-core/assets/media-prices.json`, embedded with
`include_str!` and parsed once into `catalogue::media_prices::MediaPrices` (a `LazyLock`). Format:
[contracts/media-prices.md](contracts/media-prices.md). Each entry names a provider id, a model (an
exact id, or a prefix ending in `*`), a kind, and one or more prices: token rates (USD per 1M input
/ output tokens), per image (by variant, with a default), per 1M characters, per audio minute. Each
entry carries `source` (the provider's pricing page) and `as_of`; the file carries its own `as_of`.

First coverage: the default model of every built-in media provider plus the common alternatives —
OpenAI (`gpt-image-1`, `dall-e-3`, `tts-1`, `tts-1-hd`, `gpt-4o-mini-tts`, `whisper-1`,
`gpt-4o-transcribe`, `gpt-4o-mini-transcribe`, `text-embedding-3-small/large`), ElevenLabs, xAI,
Groq, Mistral, Gemini, Voyage, Cohere, Together, Venice, Hyperbolic, Hugging Face, Bedrock (Titan,
Nova Canvas, Cohere). Prices are looked up from the providers' pages when the table is written
(implementation task), never guessed.

A unit test, `every_default_media_model_is_priced_or_local`, walks every `*Provider` enum and fails
when a provider's default model is neither local nor in the catalogue nor in the table, so a new
media provider cannot ship unpriced by accident.

**Rationale**: The spec's option B. A JSON asset beside `assets/catalogue/` follows the catalogue's
own pattern, needs no new dependency, and is reviewed as data in a release diff.

**Alternatives considered**: A Rust `const` table — harder to diff and to review as prices; TOML —
not otherwise used for assets; fetching prices at runtime — violates the catalogue's "startup makes
no network request" rule and Principle III.

## R6. Pricing order and the price source

**Decision**: `usage::Pricer` gains `media_cost(kind, provider, model, variant, amounts, tokens)`:

1. A local record: `Some(0.0)`, source **Local** (FR-010).
2. Reported tokens and the catalogue has a cost for `(provider, model)`: `estimate_cost`, source
   **Catalogue** (FR-009, the catalogue is first per FR-012).
3. Reported or measured tokens and the table has token rates: those rates, source **Table**.
4. The table has a unit price for the record's unit (image variant → default image price;
   characters; audio minutes): `amount × price`, source **Table**.
5. Otherwise `None` — unpriced (FR-011).

`ModelRow.price_source` (`catalogue | table | local | none`) tells the page which applied (FR-020).
Completions keep the 018 path unchanged (SC-006).

**Rationale**: One function, one order, tested in isolation; prices are read at report time, as for
chat (spec assumption).

## R7. Storage: extend `usage_event`

**Decision**: New columns on `usage_event`, not a new table:

`kind TEXT NOT NULL DEFAULT 'text'`, `variant TEXT`, `images INTEGER NOT NULL DEFAULT 0`,
`characters INTEGER NOT NULL DEFAULT 0`, `audio_ms INTEGER NOT NULL DEFAULT 0`,
`measured INTEGER NOT NULL DEFAULT 0`, `local INTEGER NOT NULL DEFAULT 0`, `call_id TEXT`.

SQLite: `add_column_if_missing` in the usage schema init (an #82 entry, per CLAUDE.md's key
conventions). PostgreSQL: `0015_media_usage.sql`. Every grouping in `usage_groups` also groups by
`kind`, `variant` and `local` and sums the three unit columns, so the existing Rust pricing loop can
price each group. One index is added: `usage_event_call ON usage_event (call_id) WHERE call_id IS
NOT NULL` (R10).

**Rationale**: Every figure on the page (headline, daily, breakdowns, costliest, conversation
details) already comes from `usage_groups`; adding columns keeps one query path (Principle II). Old
rows read as kind `text` with zero units, so SC-006 holds without a data migration.

**Alternatives considered**: A `media_usage_event` table — doubles every query and the merge logic.

## R8. Activities and "accounting since"

**Decision**: `UsageActivity::Indexing` (`"indexing"`, label "Indexing") is appended to
`UsageActivity::ALL` (order is display order and is append-only like `ToolCategory`). A second state
key, `usage.media_accounting_since`, is set once at the first startup after the upgrade.
`UsageReport.media_accounting_since` is returned beside `accounting_since`; the chart fades a day
before it when the page is unfiltered, filtered to a media kind, or filtered to Indexing (FR-017).

## R9. Kind dimension and filter

**Decision**: `schema::usage::UsageKind { Text, Image, Speech, Transcription, Embedding }` with
`as_str`/`label`/`unit`. `UsageQuery.kind: Option<UsageKind>`; `UsageGrouping::Kind`. The report
gains `by_kind: Vec<KindRow>` (empty when filtered to a kind), `summary.kind_share_of_all` when
filtered to one, and each `DailyUsage` gains `by_kind` for the tooltip. The two filters compose with
`AND`. HTTP: `?kind=` beside `?activity=` on both usage routes.

## R10. Cost on a chat step (Story 4)

**Decision**: Media records made by a tool carry `call_id` = the `execute_python` call's id (R2), the
same id the history's `ToolCall`/`ToolResult` entries hold. A new route,
`GET /agents/{id}/usage/calls?ids=<id>,<id>` (≤ 50 ids), returns the priced media records per call
id, under the same access rule as the usage page (`can_view_agent`, 018 FR-020). The WebUI asks for
the ids of a turn's steps when a step is expanded and draws one line per media call in the step's
details.

**Rationale**: No change to history or to the tool's result (which the model and every viewer see);
cost is read at view time with current prices, like the usage page.

**Alternatives considered**: Putting the cost in the tool result — the model would see money figures
and the redaction/visibility rules would differ from the usage page.

## R11. End-to-end harness

**Decision**: A `dummyplug` provider variant is added to the four media provider enums: local, free,
no credentials, deterministic — a 1×1 PNG, a 100 ms silent WAV, the transcript
`dummyplug transcript`, and hash-derived unit vectors. It fills `Reported` with fixed figures for
every kind so both the reported and the measured paths are exercised (an `embed` call reports
tokens; a `generate` call reports none, so the image count is measured). The quickstart drives it
through an agent on the dummyplug chat provider. Pricing of a real provider is checked by unit tests
over the shipped table, plus one quickstart step marked **live** (OpenAI image + embedding), as the
constitution allows for token accounting.

**Rationale**: The constitution makes dummyplug the default e2e harness; there is no offline media
provider today, and the local Whisper/Kokoro/fastembed models download weights.

**Alternatives considered**: A fake OpenAI-shaped HTTP server — the media base URLs are fixed per
provider (`media_base_url`), so it would need a base-URL override just for tests.

## R12. Not counted

Model downloads (Ollama pulls, Whisper/Kokoro/fastembed weights), the provider connection test,
embeddings attributed to no agent. Vision stays a completion (018).

## R13. UI design

**Decision**: FigCraft was unreachable in this session, so the design is an ASCII mockup in
[contracts/webui.md](contracts/webui.md), to be reviewed before any UI code (constitution: design
first, any medium). New visual pieces (the Kind tab and filter chips, units in breakdown rows, the
step cost line) go into `design/` with stories first, then `webui/`.
