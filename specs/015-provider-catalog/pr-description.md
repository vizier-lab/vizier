# feat!: catalogue-driven providers and models, custom providers, and removing rig [**breaking**]

Closes #69.

## What changes

- **Providers and models come from the [models.dev](https://models.dev) catalogue** (230 providers, ~8,900 models), not hard-coded lists. Any catalogue provider that speaks OpenAI chat, OpenAI Responses, Anthropic, Gemini or Ollama works with no code change. A snapshot is embedded at build time; a person's visit to the pickers or Settings › Providers may trigger a lazy, conditional refresh in the background (cached in SQLite). Agents, tasks and dreams never do, and startup makes no request. `catalogue.refresh: false` turns it off.
- **rig is gone.** `src/llm/` is vizier's own provider layer: five dialects, a shared `reqwest` transport, `RetryingModel` (which now honours `Retry-After`), credential sources (static key, Copilot token exchange, ChatGPT account), and media clients for STT, TTS, image generation and embeddings. Every dialect's wire format is pinned by golden fixtures recorded from rig 0.39. Intentional differences are listed in `pr-notes.md`.
- **Accurate limits and capabilities.** Context window and max output come from the catalogue, and the agent's own values override them. Images go straight to vision models and become a short note for others, so read-image needs no separate vision model. A save warns, without blocking, about models without tool calling, images on a model that can't see them, and unknown models. `vizier agent ps` and agent health show the effective context window and where it came from.
- **Custom providers.** Add an OpenAI-compatible or Anthropic endpoint at any URL, with its own model list. Fetch the list from the server; blank fields are filled from the catalogue. Edited rows survive a re-fetch.
- **Settings › Providers.** Shows the catalogue status with a Refresh button and the connected providers: saved keys, keys found in env vars, which agents use each, and whether local and custom providers are reachable. You can search and connect any provider, test before saving, view a provider's models, and get a delete warning listing the affected agents. Changing or deleting credentials restarts the agents that use them.
- **One agent form** for create and edit (`refactor:` commit), with a shared provider/model picker. The picker has search, connection groups, a virtualised model list with capabilities and prices, free-typed ids and inline connect. The form also gains an Advanced section with catalogue placeholders, the checkpoint threshold shown in tokens, a self-configuring read-image section, media key hints, and the model card and warnings on Review.
- **Estimated cost** per agent, day and channel type in the usage view. It is `null`/"—", never `$0`, when no price is known.

## Breaking changes

- **Galadriel and Mira are dropped.** Their saved credentials are deleted on upgrade. Agents on them show `provider "galadriel" is not available; pick another provider` until moved.
- **YAML `providers` is removed.** A leftover section is ignored, with a warning naming the env var to set. Credentials live in Settings › Providers or env vars.
- **Provider ids are renamed** to catalogue ids (`gemini` → `google`, `together` → `togetherai`, `copilot` → `github-copilot`, `opencode_zen` → `opencode`, `llama_cpp` → `llama-cpp`, …). Saved credentials and agent configs are migrated once on startup. Legacy names are still accepted by the API and in configs, and the env vars vizier read before are still honoured.
- The per-agent `embedding.api_key` / `embedding.base_url` fields are ignored. Embeddings use the matching provider's credentials.

## Upgrade

Nothing to do. On first start, `migrate_provider_ids_v1` rewrites `provider_config` rows and agent configs, turns a legacy `custom` row (or `CUSTOM_BASE_URL`/`CUSTOM_API_KEY`) and unmatched base URLs into custom providers, and logs each change. The migration is idempotent.

## Testing

- **Baseline (T001)**: `cargo test` 303 passed; clippy 98 warnings; typecheck ok.
- **Now**: `cargo test` 395 passed, 6 ignored (the live smoke tests); clippy 0 errors, 87 warnings, none in new code; `npm run typecheck` ok.
- **Dependencies**: `cargo tree | grep -c rig-core` is 0, and `cargo tree -e normal | wc -l` went 1989 → 1912.
- **End to end** (constitution gate) on the final build: `specs/015-provider-catalog/fixtures/e2e/`, against `fixtures/stub_llm.py` and dummyplug:
  - upgrade 19/19;
  - retry / `Retry-After` 3/3;
  - capabilities 8/8;
  - catalogue and lazy refresh 27/27;
  - custom providers and providers API 24/24.

  The 008 and 014 dummyplug scripts pass unchanged.
- **WebUI**: walked in Chrome (edit form, picker, inline connect, Advanced, read-image, Settings › Providers, custom provider flow, usage).
- **Live providers**: tested by the project owner with their keys (`llm_smoke_*` and the WebUI); all good.
- **Release dry run**: <!-- T099 --> `release-and-publish.yml` with `dry_run: true` is pending. Locally, `cargo package --list` contains `assets/catalogue/api.json`.

Details, deviations from the task list, and golden-fixture differences: `specs/015-provider-catalog/pr-notes.md`.

## Suggested follow-up

A PATCH clarification to the constitution's Distribution constraint (research R13). The project owner ruled that a lazy, person-triggered metadata refresh with a complete offline snapshot and an off switch complies with Principle III. The constraint's literal wording ("features that fetch something at runtime MUST be gated behind explicit user configuration") reads more strictly, and recording the ruling would stop future reviews re-litigating it.

## Commits

- `feat!:` with `[**breaking**]` for the rig cutover, dropped providers and YAML `providers`.
- `refactor:` for the form merge.
- `chore:` for catalogue seed refreshes.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
