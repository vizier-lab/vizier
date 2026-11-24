# Tasks: Usage and Spend of Images, Audio and Embeddings

**Input**: Design documents from `specs/065-media-usage-accounting/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: Included. The constitution requires the dummyplug end-to-end checks scripted in
[quickstart.md](quickstart.md), and the quickstart names the unit checks; each story's tests are
listed with it.

**Organization**: one phase per user story (US1–US4 from spec.md), in priority order.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: the user story the task belongs to (US1–US4)
- Paths are from the repository root. `core/` below means `crates/vizier-core/src/`.

---

## Phase 1: Setup

- [X] T001 Create the matching submodule branches with `just checkout 065-media-usage-accounting` (design, webui, landing, dummyplug), then confirm with `just status`
- [ ] T002 Add a checklist entry to issue #82 naming the `usage_event` `add_column_if_missing` calls added in T005 and the `usage.media_accounting_since` marker (T010), and what reads them (`gh issue comment 82`); the `gh` call posts publicly, so ask the user before posting

---

## Phase 2: Foundational (blocks every story)

**Purpose**: the kind, the new columns and the scope every media record needs.

- [X] T003 Add `UsageKind { Text, Image, Speech, Transcription, Embedding }` with `ALL`, `as_str`, `label`, `FromStr`, serde snake_case and utoipa schema, and append `UsageActivity::Indexing` (`"indexing"`, label "Indexing") to the end of `UsageActivity::ALL`, in `core/schema/usage.rs`
- [X] T004 Extend `UsageEvent` in `core/schema/usage.rs` with `kind`, `variant: Option<String>`, `units: MediaUnits { images, characters, audio_ms }`, `measured`, `local`, `call_id: Option<String>` (data-model §3); fill defaults (`Text`, zeros, `false`, `None`) at the existing constructors in `core/agents/agent/model/mod.rs`, `core/dependencies.rs` (legacy carry-over) and the tests in `core/usage/mod.rs`
- [X] T005 Add the eight columns and the partial index `usage_event_call (call_id) WHERE call_id IS NOT NULL` to `init_usage_schema` with `add_column_if_missing` in `core/storage/sqlite/mod.rs`, and write/read them in `record_usage`, `insert_legacy_usage` and `conversation_usage` in `core/storage/sqlite/usage.rs`
- [X] T006 [P] Write `core/storage/sql/postgres/migrations/0015_media_usage.sql` with the same eight columns (with defaults) and the partial index, and register it wherever the migration list is enumerated in `core/storage/sql/postgres/`
- [X] T007 Extend `UsageScope` in `core/agents/agent/model/usage.rs`: `UsagePurpose::Media` (activity = session's `usage_activity()`, else `Indexing`), `call_id: Option<String>`, the constructor `UsageScope::media(session)`, a `with_call(call_id)` builder, and the `tokio::task_local!` `MEDIA_SCOPE: UsageScope` with a helper `media_scope() -> UsageScope` (the ambient one, or `media(None)`)
- [X] T008 Move the spawned write out of `VizierModel::record` into `core/usage/record.rs` as `pub fn spawn_record(storage: Arc<VizierStorage>, event: UsageEvent)` (warn-only on failure), add `pub(crate) mod record;` in `core/usage/mod.rs`, and make `VizierModel::record` call it
- [X] T009 Add `Reported { input_tokens, output_tokens, images, characters, audio_ms, served_model }` (all `Option`, `Default`) and the output wrappers `GeneratedImage { bytes, mime, reported }`, `Speech { audio, reported }`, `Transcript { text, reported }`, `Embeddings { vectors, reported }` in `core/llm/media/mod.rs`
- [X] T010 Set state key `usage.media_accounting_since` once at startup (only when absent) beside `USAGE_ACCOUNTING_SINCE` in `core/dependencies.rs`, as `VizierDependencies::USAGE_MEDIA_ACCOUNTING_SINCE`

**Checkpoint**: `cargo test -p vizier-core` passes with old rows reading as kind `text`.

---

## Phase 3: User Story 1 — Every media call is counted (P1) 🎯 MVP

**Goal**: one usage record per answered image, speech, transcription and embedding request, with kind, amount, provider/model, activity and conversation.

**Independent test**: [quickstart.md](quickstart.md) steps 1–6 and 9–10 on a dummyplug agent: each media call appears under the right kind, activity and conversation, with its amount.

### Measuring and handles

- [X] T011 [US1] Add the measuring helpers to `core/usage/record.rs`: `wav_duration_ms(&[u8]) -> Option<u64>` (RIFF header), `pcm_duration_ms(len, rate)`, `estimate_tokens(texts) = Σ ceil(bytes/4)`, `char_count(text)`, and `media_event(scope, agent_id, kind, provider, model, local, reported, measured_units, took) -> UsageEvent` that prefers reported figures and sets `measured` when any amount was measured (research R4)
- [X] T012 [US1] Change `VizierImageGenModel::generate` to return `Result<GeneratedImage>`; give `VizierImageGen` `agent_id`, provider id string, model, `storage`, and make `VizierImageGen::generate(prompt, size, references, scope: &UsageScope)` record `images = reported or 1` and `variant` = requested or default size (+ quality when set) via `spawn_record`, in `core/image_generation/mod.rs`; update its constructor's caller in `core/agents/agent/mod.rs` to pass `agent_id`
- [X] T013 [P] [US1] Change `VizierTtsModel::generate_speech` to return `Result<Speech>`; `VizierTts::generate_speech(.., scope)` records `characters` and `audio_ms` (WAV header, or `gemini_pcm_rate`), `local` for Kokoro, in `core/tts/mod.rs`
- [X] T014 [P] [US1] Change `VizierSttModel::transcribe` to return `Result<Transcript>`; `VizierStt::transcribe(.., scope)` records `audio_ms` (WAV header, else reported), `local` for Whisper, in `core/stt/mod.rs`
- [X] T015 [P] [US1] Change `VizierEmbeddingModel::embed_texts` to return `Result<Embeddings>` (keep `embed_text` as a default over it); `VizierEmbedder` gains agent id, provider, model, `local` (Local/fastembed, Ollama) and storage, and records one event per `embed_texts` call with `MEDIA_SCOPE`'s scope (`media_scope()`), tokens reported or `estimate_tokens`, in `core/embedding/mod.rs` and `core/embedding/fastembed.rs`; pass the new fields from `core/agents/supervisor.rs`
- [X] T016 [US1] Update every provider client to the new return types with `Reported::default()` (parsing comes in T024–T028): `core/llm/media/{openai_shaped,gemini,cohere,bedrock,ollama,venice,hyperbolic,huggingface}.rs`, `core/stt/whisper*.rs`, `core/tts/kokoro*.rs` and the golden tests in `core/llm/media/golden_tests.rs` — done by giving each trait a `*_reported` default method instead (research R1, as built); only OpenAI-shaped clients override it

### Scope at the call sites

- [X] T017 [US1] Add `ToolContext::usage_scope() -> UsageScope` (session, turn id, round, current `execute_python` call id) in `core/agents/tools/mod.rs`, with the call id/turn/round fields set where `prompt_with` builds each round's `ToolContext` in `core/agents/agent/mod.rs` — done without a new `ToolContext` field: the ambient `MEDIA_SCOPE` carries turn, round and call id (research R2, as built)
- [X] T018 [US1] Wrap each tool call `RouterBridge` dispatches in `MEDIA_SCOPE.scope(ctx.usage_scope(), fut)` in `core/agents/tools/python/bridge.rs`, so embeddings made by memory tools carry the turn's conversation
- [X] T019 [US1] Pass `ctx.usage_scope()` to the media handles in `core/agents/tools/image_gen/mod.rs`, `core/agents/tools/tts/mod.rs`, `core/agents/tools/stt/mod.rs`
- [X] T020 [US1] Pass the turn's scope to the spoken-reply TTS call (~line 403) and the voice-message STT call (~line 734) in `core/agents/agent/mod.rs`, and wrap the turn's automatic memory recall in `MEDIA_SCOPE.scope(..)` there
- [X] T021 [P] [US1] Pass `UsageScope::media(Some(session))` for the call's conversation to STT and TTS in `core/live_voice/engines.rs`
- [X] T022 [P] [US1] Wrap each dream stage's tool calls in `MEDIA_SCOPE.scope(..)` with the stage's session in `core/scheduler/dream/` (where `dream_call` dispatches), so dream embeddings read **Dreams**

### Offline harness and tests

- [X] T023 [US1] Add a `Dummyplug` variant to `ImageGenProvider`, `TtsProvider`, `SttProvider` and `EmbeddingProvider` (no credentials, local, `default_model` `dummyplug-image|speech|transcribe|embed`) in `core/schema/agent.rs`, and implement the four models in a new `core/llm/media/dummyplug.rs`: 1×1 PNG with no reported count, a 100 ms silent 16 kHz WAV, transcript `dummyplug transcript`, hash-derived unit vectors reporting `input_tokens = estimate_tokens(texts)`; wire them into the four handle constructors — done early: the handles needed it to compile
- [X] T024 [US1] Add `media_usage` cases to `crates/vizier-core/tests/dummyplug_e2e.rs` covering quickstart steps 1–6 and 10 (indexing at start, image, speech, transcription, memory write + search under `chat` in the topic, measured vs reported, a failed image call that records nothing) — `media_usage_is_recorded_for_every_kind`; steps 7 and 8 checked at the storage level; step 10 (failure) not yet
- [ ] T025 [US1] Run quickstart steps 1–6, 9 (live voice in the WebUI) and 10 against `just run` with a dummyplug agent, and record the outcome in the PR description

**Checkpoint**: every media call is in `usage_event`; the existing page already counts their calls (costs still null until US2).

---

## Phase 4: User Story 2 — Know what media cost (P1)

**Goal**: media records priced from the catalogue, then the shipped table; local is free; unpriced amounts reported in their units.

**Independent test**: unit checks in quickstart (`media_prices_parse`, `every_default_media_model_is_priced_or_local`, `pricer_media_order`, `image_variant_falls_back_to_default`) and the live step 12.

### Reported figures (provider parsing, research R3)

- [X] T026 [P] [US2] Parse `usage.prompt_tokens` (embeddings), `usage.input_tokens`/`output_tokens` (gpt-image), and transcription `usage` of type `tokens` or `duration` (seconds → `audio_ms`) in `core/llm/media/openai_shaped.rs`, with fixtures in `core/llm/media/fixtures/`
- [ ] T027 [P] [US2] Parse Voyage `usage.total_tokens` and Cohere `meta.billed_units.input_tokens` in `core/llm/media/cohere.rs` (and the Voyage path, wherever it lives in `openai_shaped.rs`), with fixtures — **skipped**: Voyage/Cohere/Bedrock/Ollama figures are either unpriced or equal to what the meter measures; follow-up
- [ ] T028 [P] [US2] Parse Bedrock Titan `inputTextTokenCount` in `core/llm/media/bedrock.rs` and Ollama `prompt_eval_count` in `core/llm/media/ollama.rs`, with fixtures — **skipped**, same reason as T027
- [ ] T029 [P] [US2] Read ElevenLabs' `character-cost` response header into `Reported.characters` in the ElevenLabs TTS client (`core/llm/media/openai_shaped.rs` or its own file), and Groq's transcription duration where present — **skipped**: ElevenLabs is priced by characters, which the meter already counts exactly; follow-up

### Price table and pricing

- [X] T030 [US2] Write `crates/vizier-core/assets/media-prices.json` per [contracts/media-prices.md](contracts/media-prices.md): the default model of every built-in media provider plus the common alternatives listed in research R5, each price taken from the provider's pricing page (cite it in `source`, date in `as_of`) — never estimated
- [X] T031 [US2] Implement `core/catalogue/media_prices.rs`: parse the embedded file once (`LazyLock`), skip and `warn!` a bad entry, `lookup(provider, model, kind) -> Option<&MediaPrice>` (exact before prefix, longest prefix, `ProviderId` aliases); register the module in `core/catalogue/mod.rs`; unit tests `media_prices_parse` and `prefix_and_exact_lookup`
- [X] T032 [US2] Add `PriceSource { Catalogue, Table, Local, None }` in `core/schema/usage.rs` and `Pricer::media_cost(kind, provider, model, variant, local, units, tokens) -> (Option<f64>, PriceSource)` in `core/usage/mod.rs` with the order of research R6; unit tests `pricer_media_order` and `image_variant_falls_back_to_default`
- [X] T033 [US2] Extend `UsageGroup` with `kind`, `variant`, `local` and summed `units`, and group by them in every `usage_groups` grouping in `core/storage/sqlite/usage.rs` and `core/storage/usage.rs`; route each group's cost through `media_cost` for non-text kinds in `Tally::add`, `daily`, `by_model` and `costliest` in `core/usage/mod.rs`
- [X] T034 [US2] Extend `UnpricedNote` with `units: BTreeMap<UsageKind, Units>` and never list a local row in it, in `core/schema/usage.rs` and `by_model` in `core/usage/mod.rs`
- [X] T035 [US2] Add the guard test `every_default_media_model_is_priced_or_local` (walk the four provider enums' `default_model`; a short `KNOWN_UNPRICED` list, each with a reason, is the only exception) in `core/catalogue/media_prices.rs`
- [ ] T036 [US2] Run quickstart step 12 (live, marked: OpenAI `gpt-image-1` + `text-embedding-3-small`) and check the image cost against OpenAI's published price within 1% (SC-002); note the result in the PR — **open: needs an OpenAI key**

**Checkpoint**: the existing page's headline and breakdowns include media spend.

---

## Phase 5: User Story 3 — See media on the usage page (P2)

**Goal**: a Kind breakdown and filter, units in rows, the Indexing activity, the media start date in the chart.

**Independent test**: quickstart step 8 (filters compose) and the reviewed mockup in the running WebUI.

### Backend

- [X] T037 [US3] Add `UsageQuery.kind: Option<UsageKind>` (filter in every query's `WHERE`) and `UsageGrouping::Kind` in `core/storage/usage.rs` and `core/storage/sqlite/usage.rs`
- [X] T038 [US3] Add the report shapes of data-model §9 (`Units`, `BreakdownRow.units`, `KindRow`, `UsageReport.by_kind`/`kind`/`media_accounting_since`, `ModelRow.kind`/`price_source`, `UsageSummary.kind_share_of_all`, `DailyUsage.by_kind`, conversation call `kind`/`units`/`variant`/`measured`) in `core/schema/usage.rs`, and fill them in `build_report` and `conversation_usage` in `core/usage/mod.rs` (`by_kind` empty when filtered to a kind; `kind_share_of_all` under the same activity filter)
- [X] T039 [US3] Accept `kind` (and `indexing` for `activity`) on `UsageParams` and `ConversationUsageParams`, answer `400 unknown_kind`, and pass it through `service::agents::usage`/`usage_conversation` in `crates/vizier-http/src/api/v1/agents/mod.rs` and `core/service/agents.rs`
- [ ] T040 [US3] Regenerate the WebUI's `webui/app/api/schema.d.ts` with `scripts/openapi.mjs` and add `kind` to the usage endpoint wrappers in `webui/app/api/endpoints/usage.ts`
- [ ] T041 [US3] Add `media_usage` e2e case for quickstart step 8 (kind filter, `by_kind: []`, `kind_share_of_all`, empty state with `kind=image&activity=dreams`) in `crates/vizier-core/tests/dummyplug_e2e.rs`

### Design, then WebUI (constitution: design first)

- [ ] T042 [US3] Get the user's review of the mockup in [contracts/webui.md](contracts/webui.md) (or draw it on the usage page of `Vizier shell.figcraft` if the FigCraft MCP is reachable) before any UI code
- [ ] T043 [P] [US3] Add an optional `units` second line to `BarList` rows, with stories for a media row and a mixed row, in `design/src/components/BarList/`
- [ ] T044 [US3] Add the units formatters (`images`, `140k chars`, `2.5 h audio`, tokens) and the kind labels, with vitest cases, in `webui/app/lib/usage.ts` and `webui/app/lib/usage.test.ts`
- [ ] T045 [US3] On the usage page add the Kind filter row (design `FilterTabs`, kept in the URL like `activity`), the **Kind** breakdown tab (hidden while filtered to a kind), units on breakdown rows, `price_source` ⓘ ("Estimated from Vizier's price table (as of <date>)") and "free · local" for local rows, the per-kind lines in the day tooltip, fading days before `media_accounting_since`, the kind-share figure in the headline, and media calls in the conversation details, in `webui/app/routes/usage.tsx`
- [ ] T046 [US3] Bump the `design` pin in `webui`, then the `webui` pin in vizier (order design → webui → vizier), and run `cd webui && npm run typecheck` and its vitest suite

**Checkpoint**: the page answers "what did images cost this month" in two interactions (SC-003).

---

## Phase 6: User Story 4 — Media use is visible where it happens (P3)

**Goal**: a chat step that made media calls shows their model, amount and cost.

**Independent test**: quickstart step 7, then expand the image step in the WebUI chat.

- [X] T047 [US4] Add `UsageStorage::media_calls(agent_id, call_ids) -> Vec<UsageEvent>` (uses `usage_event_call`) in `core/storage/usage.rs` and `core/storage/sqlite/usage.rs`, and `usage::media_calls` pricing each row into `MediaCall` (data-model §9) in `core/usage/mod.rs`
- [X] T048 [US4] Add `GET /agents/{agent_id}/usage/calls?ids=` (1–50 ids, `400 invalid_ids`, `can_view_agent` else `404`) with its OpenAPI entry in `crates/vizier-http/src/api/v1/agents/mod.rs` and the service function in `core/service/agents.rs`; add its conformance vector in `crates/vizier-http/tests/conformance/groups/` (the agents group) and remove it from `tests/conformance/pending.txt` if listed
- [ ] T049 [US4] Add the `media_usage` e2e case for quickstart step 7 (`/usage/calls` returns the step's image record; an id without media calls is absent) in `crates/vizier-core/tests/dummyplug_e2e.rs`
- [ ] T050 [P] [US4] Add an optional `meta` list slot to the step details of `TrailCall`, with a story, in `design/src/components/TrailCall/`
- [ ] T051 [US4] Fetch `/usage/calls` for a step's `execute_python` call id when it is expanded (`webui/app/api/endpoints/usage.ts`) and draw one meta line per media call (`openai / gpt-image-1 · 1 image (1024×1024, high) · $0.17`) in the chat trail under `webui/app/screens/chat/`; draw nothing when there is no entry

---

## Phase 7: Polish & cross-cutting

- [X] T052 [P] Write `docs/src/configuration/usage.md` (what is counted per kind, reported vs measured, Indexing, local models are free, the shipped price table and its `as_of`, what is not counted) and add it to `docs/src/SUMMARY.md`
- [X] T053 [P] Update CLAUDE.md's **Usage accounting** section: media handles record (`spawn_record`), `MEDIA_SCOPE`, `UsageKind`, the price table and its guard test; remove "Not recorded: STT, TTS, image generation, embeddings"
- [ ] T054 Seed 100k completion and 500k embedding rows on one agent and time the 90-day report (SC-005, < 2 s); add an index only if it misses
- [ ] T055 Run quickstart step 11 (upgrade from a pre-065 workspace: chat figures unchanged, `media_accounting_since` set) with `just run`
- [ ] T056 Run `cargo clippy --workspace`, `cargo test --workspace`, `cargo test -p vizier-core --no-default-features` and `--no-default-features --features provider-media` (never `cargo fmt`)

---

## Dependencies & execution order

- **Setup (T001–T002)** → **Foundational (T003–T010)** → stories.
- **US1 (T011–T025)** depends on Foundational. It is the MVP: every call is recorded.
- **US2 (T026–T036)** depends on US1's records (T011–T016). T026–T029 only need T016.
- **US3 (T037–T046)** depends on US2's group fields (T033) for costs; T042 (mockup review) gates T043–T046.
- **US4 (T047–T051)** depends on US1's `call_id` (T017–T019) and US2's pricing (T032); independent of US3.
- **Polish (T052–T056)** after the stories it documents or checks.

Within US1: T011 → T012–T015 → T016 → T017–T022 → T023 → T024–T025.

## Parallel opportunities

- T006 beside T005 (different backends).
- T013, T014, T015 after T012 (one handle each).
- T021, T022 beside T019–T020 (different call sites).
- T026–T029 (one provider file each).
- T043 and T050 (design components) beside backend work once T042 is done.
- T052, T053 (docs) at any point after the behaviour is settled.

```text
# US1 handles, together:
T013 core/tts/mod.rs   T014 core/stt/mod.rs   T015 core/embedding/mod.rs
# US2 provider parsing, together:
T026 openai_shaped.rs  T027 cohere.rs  T028 bedrock.rs + ollama.rs  T029 ElevenLabs/Groq
```

## Implementation strategy

1. **MVP = Setup + Foundational + US1**: every media call is recorded and counted on the existing
   page; validate with T024–T025.
2. **US2**: prices appear (headline includes media spend); validate with T031–T036.
3. **US3**: the Kind breakdown and filter, after the mockup review.
4. **US4**: the step cost line.
5. Polish: docs, performance, upgrade check, full lint and test run.

## Notes

- Never run `cargo fmt` (the workspace is not rustfmt-clean).
- Commit with conventional messages; across repos use `just commit` and keep the order design → webui → vizier.
- Prices in T030 come from the providers' pages. A default model with no published price is never
  guessed: it goes on the guard test's explicit `KNOWN_UNPRICED` list (T035) with a one-line reason.
