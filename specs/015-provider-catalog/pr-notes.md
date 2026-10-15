# PR notes (015-provider-catalog) — collected during implementation

## Baseline (T001, on 30a1d42 before any change)

- `cargo build`: ok
- `cargo test`: 303 passed, 0 failed
- `cargo clippy`: 98 warnings, 0 errors (pre-existing; mostly `collapsible_if`)
- `cd webui && npm run typecheck`: ok
- `cargo tree -e normal | wc -l`: 1989

## Deviations from tasks.md (with reasons)

- **T028 `type ProviderVariant = ProviderId` alias**: not possible — dozens of `match` arms use
  `ProviderVariant::openai`-style enum patterns, and a string newtype can't be a const pattern.
  `ProviderId` was added beside the enum in Phase 2 and the enum is deleted at the cutover (T049).
- **T013 `CredentialSource`**: gained a `RequestInfo` argument (Copilot's `X-Initiator` and
  `copilot-vision-request` depend on the request) and a `base_url()` hook (a Copilot token names
  its own API host). `invalidate()` returns whether a cached token was dropped.
- **T014**: `complete_until(req, deadline)` is a `CompletionModel` trait method (default ignores the
  deadline; `RetryingModel` overrides it), so `VizierModel` can stay a plain `Arc<dyn CompletionModel>`.
- **T018 Anthropic caching**: block-level `cache_control` on the last tool, the system prompt and the
  last message block, as the task specifies. rig 0.39's `with_automatic_caching` sent a single
  top-level `cache_control` instead; block-level breakpoints work on every Anthropic-compatible
  endpoint (MiniMax, opencode, Azure Foundry).
- **T020 Ollama tool-call ids**: the function name, as rig did — not a synthesised `call_<n>`. Ollama
  answers tool results by `tool_name`, so a synthesised id would break the round trip.
- **T023 Cohere embeddings**: `/v1/embed` with `texts`/`input_type`, which is what rig 0.39 actually
  sent (the task said `/v2/embed`); behaviour unchanged.
- **OpenAI chat**: system and assistant text are sent as plain strings, not part arrays (several
  OpenAI-compatible servers, DeepSeek among them, accept only strings). Single-text user messages
  were already strings.
- **T031 order**: the golden capture was written and run before the media clients (T022/T023) so the
  dialects could be built against rig's real wire output; media was captured afterwards, still with
  rig present.

## Golden fixtures: intentional differences from rig 0.39

Each is a named normaliser in `src/llm/dialect/golden_tests.rs` or noted in
`src/llm/media/golden_tests.rs`; everything else matches rig byte-for-byte as JSON values.

| Dialect | Difference | Why |
|---|---|---|
| OpenAI chat | system/assistant content as strings | compatibility (above) |
| DeepSeek | `tool_choice: "none"` is sent | rig's DeepSeek client dropped it, so answer-only turns could still call tools |
| Anthropic | one joined system block; block-level cache breakpoints | see T018 |
| Anthropic, Gemini | a later system message becomes `<system>…</system>` user text | contract S8; rig hoisted it to the top, which also defeats caching |
| Gemini | `maxOutputTokens` sent when `max_tokens` is set | rig only sent it when a generation config was already present |
| Gemini | key in `x-goog-api-key` header, not `?key=` | keeps the key out of URLs and logs |
| Ollama | output cap as `options.num_predict` | rig sent a top-level `max_tokens`, which Ollama ignores |
| Responses | leading system → `instructions`, later → developer messages; `store: false`; function calls carry only `call_id` | contract S5/S8; with nothing stored an item id can't be resolved (rig's replay of vizier history also failed with "call_id is required") |
| Hugging Face | requests go to `/hf-inference/models/{model}` | rig dropped the inference route segment |
| all | `null` and absent fields are equivalent | |
