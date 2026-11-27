# Contract: WebUI (usage report)

**Design status**: ASCII mockup below, **approved by the person on 2026-11-27** (constitution:
design before UI code; FigCraft was unreachable, so the mockup is ASCII). There is no new component and no
layout change: the change is the wording of the existing second line under a model row in
"Where it went", which `priceNote` builds today.

## Mockup — "Where it went", by model

```
 Model                                         Calls    Tokens     Cost   Share
 ─────────────────────────────────────────────────────────────────────────────
 anthropic / claude-sonnet-4-5                   42    1.2M      $4.10   ████████
   models.dev
 venice / qwen-image-2-pro                        6       —      $0.30   ██
   Image · 6 images · Venice
 openrouter / openai/gpt-4o-mini-tts              3     900      $0.01   ▏
   Speech · 1.2k characters · OpenRouter · models.dev says $0.02
 my-proxy / claude-sonnet-4-5                    10   200k       $0.70   ██
   estimate · borrowed from anthropic
 ollama / llama3.2                               88    3.4M       free   ███████
   free · local
 chatgpt / gpt-5.1-codex                         12    410k       free   █
   included · subscription
 acme / mystery-1                                 2     12k          —   ▏
   unpriced

 No known price for acme / mystery-1; their 12k tokens are counted but not priced.
```

## Rules (`webui/app/lib/usage.ts::priceNote`)

| `price_source` | Second-line note | Cost cell |
|---|---|---|
| `local` | `free · local` | `free` |
| `subscription` | `included · subscription` | `free` |
| `borrowed` | `estimate · borrowed from <borrowed_from>` | `$…` |
| `none` | `unpriced` | `—` |
| `inference_profile` | `inference profile price` | `$…` |
| any source id | `sources[id].name` (falls back to the id) | `$…` |

When `price_alternatives` is non-empty, ` · <name> says <$cost>` is appended for the first
alternative, and the rest are reachable through the row's `title` attribute. `mediaCallLine` uses
the same note for a step's media call. `PriceSource` in `api/endpoints/usage.ts` becomes `string`,
and `schema.d.ts` is regenerated (`scripts/openapi.mjs`).

Tests: `lib/usage.test.ts` gains one case per row of the table above, plus the alternatives
suffix.
