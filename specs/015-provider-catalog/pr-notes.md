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

## Cutover (T052)

- `cargo tree | grep -c rig-core`: 0; `grep -rn "rig_core\|rig::" src`: empty.
- `cargo tree -e normal | wc -l`: 1989 → 1912.
- `cargo test`: 383 passed (the two ignored capture tests were deleted with `golden_capture.rs`).
- `cargo clippy`: 0 errors.
- Migration markers live in the `state` table (`migration.provider_ids_v1`, plus
  `migration.provider_ids_v1.map` for `migrated_from`), like vizier's other migrations, rather
  than in `global_config`, whose rows are typed.
- The provider-id migration also matches a saved base URL by host when no catalogue `api` is
  equal (`api.minimax.cn/v1` → `minimax-cn`), as the contract's examples need.

## Quickstart §1 and §5 (T053), with `fixtures/e2e/upgrade.py` and `retry.py`

A pre-015 workspace was simulated by planting legacy rows (tagged `ProviderEntryConfig` JSON
under legacy names; agents on `gemini`, `custom`, `galadriel`, `moonshot` + `api.moonshot.cn`;
a YAML `providers.openrouter`) and clearing the migration marker. 19/19 and 3/3 checks passed:

- log lines `provider 'gemini' → 'google'`, `'moonshot' → 'moonshotai-cn'`, `custom` → custom
  provider, galadriel credentials deleted, per-agent rewrites, and
  `providers.openrouter is ignored; set OPENROUTER_API_KEY or add it in Settings › Providers`;
- health: `gal-old` failed with `provider "galadriel" is not available; pick another provider`;
  `dp-old`, `cus-old`, `moon-old` alive. `gem-old` was refused at startup by Google's real API
  ("API key not valid") when its Gemini embeddings were built with the planted dummy key — the
  new request path working end to end;
- `GET /providers` lists `google` and `moonshotai-cn`, not `openrouter` or `galadriel`; the
  `custom` custom provider holds the old base URL;
- a second restart logs no migration lines (idempotent);
- `PUT /agents/gem-old` with `"provider": "gemini"` reads back `google` (old client);
- dummyplug (008/014): `tools` lists the 4 code-mode tools, the `execute_python` request
  returns `"ok": true`, `context` echoes the context block;
- §5: an agent on a custom provider whose headers make the stub answer `429` with
  `Retry-After: 2` waited ≥ 2 s (`wait_ms=2000+` in the log) and answered;
- a custom-provider agent ran a full tool loop through the stub over HTTP (`CALL …` → tool
  runs → `ECHO tool-result …`).
