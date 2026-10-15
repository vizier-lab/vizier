# Contract: Legacy Provider Name → `ProviderId`

The same table is used in two places:

- **Reads**: `ProviderId`'s `Deserialize` maps legacy names for agent JSON, provider rows and API input.
- **Startup rewrite**: a one-time migration in `dependencies.rs` rewrites `agent_config` and `provider_config` rows and logs each change at `info`.

The migration is idempotent and records completion in `global_config` under the key `migration.provider_ids_v1`.

## Name map

| Legacy `ProviderVariant` | New id | Origin | Legacy env vars still honoured |
|---|---|---|---|
| `openai` | `openai` | catalogue | `OPENAI_API_KEY` (also in catalogue) |
| `anthropic` | `anthropic` | catalogue | `ANTHROPIC_API_KEY` |
| `openrouter` | `openrouter` | catalogue | `OPENROUTER_API_KEY` |
| `deepseek` | `deepseek` | catalogue | `DEEPSEEK_API_KEY` |
| `gemini` | `google` | catalogue | `GEMINI_API_KEY` (also in catalogue) |
| `mimo` | `xiaomi` ¹ | catalogue | `XIAOMI_MIMO_API_KEY` |
| `groq` | `groq` | catalogue | `GROQ_API_KEY` |
| `mistral` | `mistral` | catalogue | `MISTRAL_API_KEY` |
| `xai` | `xai` | catalogue | `XAI_API_KEY` |
| `perplexity` | `perplexity` | catalogue | `PERPLEXITY_API_KEY` |
| `moonshot` | `moonshotai` ¹ | catalogue | `MOONSHOT_API_KEY` |
| `zai` | `zai` ¹ | catalogue | `ZAI_API_KEY` |
| `minimax` | `minimax` ¹ | catalogue | `MINIMAX_API_KEY` |
| `together` | `togetherai` | catalogue | `TOGETHER_API_KEY` |
| `cohere` | `cohere` | catalogue | `COHERE_API_KEY` |
| `huggingface` | `huggingface` | catalogue | `HUGGINGFACE_API_KEY` |
| `copilot` | `github-copilot` | catalogue | `COPILOT_API_KEY` |
| `azure` | `azure` | catalogue | `AZURE_API_KEY`, `AZURE_ENDPOINT` |
| `opencode_zen` | `opencode` | catalogue | `OPENCODE_ZEN_API_KEY` |
| `opencode_go` | `opencode-go` | catalogue | `OPENCODE_GO_API_KEY` |
| `ollama` | `ollama` | built-in | `OLLAMA_BASE_URL` |
| `llama_cpp` | `llama-cpp` | built-in | `LLAMA_CPP_BASE_URL` |
| `chatgpt` | `chatgpt` | built-in | `CHATGPT_ACCESS_TOKEN`, `CHATGPT_ACCOUNT_ID`, `CHATGPT_API_BASE` |
| `dummyplug` | `dummyplug` | built-in | — |
| `elevenlabs` | `elevenlabs` | built-in (media) | `ELEVENLABS_API_KEY` |
| `voyageai` | `voyageai` | built-in (media) | `VOYAGE_API_KEY` |
| `hyperbolic` | `hyperbolic` | built-in (media) | `HYPERBOLIC_API_KEY` |
| `custom` | `custom` | **custom provider** ² | `CUSTOM_BASE_URL`, `CUSTOM_API_KEY` |
| `galadriel` | — removed | — | — |
| `mira` | — removed | — | — |

¹ **The saved base URL decides.** If the legacy row has a `base_url` equal to (after normalising a trailing `/`) the `api` of a catalogue provider, the row maps to that provider instead. For example:

- `api.moonshot.cn` maps to `moonshotai-cn`;
- `open.bigmodel.cn/api/paas/v4` maps to `zhipuai`;
- `api.minimax.cn/…` maps to `minimax-cn`.

The same rule applies to any legacy row that carried a `base_url`, e.g. an `openai` row pointed at `token-plan-sgp.xiaomimimo.com/v1` maps to `xiaomi-token-plan-sgp`. A saved `base_url` that matches no catalogue `api` and isn't the provider's default becomes a custom provider named `<legacy>-custom`, using the OpenAI-compatible protocol and the same key. Agents are pointed at it, and the change is logged at `warn`.

² **The legacy custom provider.** The saved `Custom {api_key, base_url}` row, or `CUSTOM_BASE_URL`/`CUSTOM_API_KEY` when there is no row, becomes the custom provider `custom` with `protocol: openai_compatible` and no models listed. Agents keep their model id. If only the env vars exist, the env-backed provider is materialised once into the table, so it shows in the WebUI.

## What the rewrite touches

| Store | What changes |
|---|---|
| `agent_config.data` | `provider`, `dream_provider`, `tools.read_image.settings.provider` |
| `provider_config` | the row key (`variant`), and the tagged `ProviderEntryConfig` → `ProviderCredentials` |
| `custom_provider` | new rows from ¹ and ² |
| media settings (`tts`, `stt`, `image_gen`, `embedding` provider enums) | **nothing**: their enum names are unchanged; credentials resolve through `credentials()` (data-model §8) |

## Removed providers

An agent left on `galadriel` or `mira` (as main, dream or read-image provider) is not rewritten. It deserialises with the id kept as given, its build fails with `provider "<id>" is not available; pick another provider`, and it shows `Failed` in health and in `vizier agent ps` (Story 1, scenario 4). Saved credentials for these two are deleted, and the deletion is logged.

## YAML `providers` (FR-028)

- **The YAML section is never imported.** `migrate_providers` is deleted. Every deployment that ever started with a YAML `providers` section already had it imported into `provider_config` by that existing migration. Importing again whenever storage is empty would keep YAML working on every fresh volume, which FR-028 rules out.
- **A leftover `providers` section** is ignored at startup with a `warn`, which names the matching environment variables for each provider listed (e.g. `providers.openrouter is ignored; set OPENROUTER_API_KEY or add it in Settings › Providers`).
- **Onboarding**: `vizier onboard` stops writing `providers`. It prints the env var to set for the chosen provider, or tells the operator to add the key in Settings › Providers after first start.
