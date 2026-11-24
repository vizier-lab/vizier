# Quickstart: verifying media usage accounting

The constitution's end-to-end gate: every step runs against the real binary with an agent on the
offline `dummyplug` chat provider, and its media settings on the new `dummyplug` media provider
(research R11). Each tool step is sent as
`{"tool":"execute_python","arguments":{"intent":"test","code":"<code>"}}` with the code
JSON-escaped. Steps 1–8 are also automated in `cargo test -p vizier-core --test dummyplug_e2e
media_usage`.

## Setup

```sh
just run
```

Create agent `media` (WebUI or `POST /api/v1/agents`): provider `dummyplug`; tools `image_gen`,
`tts`, `stt` on with provider `dummyplug`; embedding provider `dummyplug`. Read the report with:

```sh
curl -s -H "Authorization: Bearer $TOKEN" \
  "http://localhost:9999/api/v1/agents/media/usage?start_date=$(date -u +%FT00:00:00Z)"
```

## Steps

1. **Indexing at start.** Right after creating the agent, the report lists `by_kind` `embedding`
   with `calls ≥ 1` and `by_activity` `indexing` (the seeded `core/soul` passages), with no
   conversation in `costliest`.
2. **Image.** In a WebUI topic send code `image_generate(prompt="a fox")`. The reply echoes a
   saved image. Report: `by_kind` `image` has `calls = 1`, `units.images = 1`; `by_model` has
   `dummyplug/dummyplug-image` with `kind: image`, `price_source: local`, `cost_usd: 0.0`; the
   activity of that row's conversation is `chat`.
3. **Speech.** Send `tts_generate(text="hello there")`. Report: `speech` `units.characters = 11`,
   `units.audio_seconds = 0.1` (the dummyplug WAV is 100 ms).
4. **Transcription.** Upload a WAV, then `stt_transcribe(filename="<name>")`. The reply contains
   `dummyplug transcript`; report: `transcription` with `units.audio_seconds` equal to the file's
   length.
5. **Embedding in a turn.** Send `memory_write(bundle="notes", path="fox", content="foxes")` then
   `memory_search(query="fox")`. Report filtered with `kind=embedding&activity=chat`: `calls ≥ 2`,
   both in this topic (`/usage/conversation?channel=<slug>&topic=<topic>&kind=embedding` lists
   them, each `measured: false` because dummyplug reports tokens).
6. **Measured vs reported.** The image call of step 2 reads `measured: true` in the conversation
   details (dummyplug reports no image count); the embedding calls read `measured: false`.
7. **Step cost.** `GET /usage/calls?ids=<execute_python call id of step 2>` returns one `image`
   record for that id; an id of a step with no media call is absent.
8. **Filters compose.** `?kind=image` → `by_kind: []`, `summary.kind_share_of_all` set,
   `costliest` only the step-2 topic. `?kind=image&activity=dreams` → empty state, `previous: null`.
9. **Live voice.** Open a live voice call on the topic (WebUI), say one sentence, let the reply play.
   Report: one more `transcription` and one more `speech` record in that topic.
10. **Failure path.** Set the image provider to `openai` with no key and run step 2 again: the tool
    fails, and the report's image `calls` is unchanged (nothing answered, FR-005).
11. **Upgrade.** Stop vizier on a workspace from before this feature, start it: chat figures for
    every earlier period are unchanged (SC-006), `media_accounting_since` is set, and days before it
    are flagged in `daily`.

## Unit checks (no binary)

- `media_prices_parse` — the embedded table parses; every entry has `source` and `as_of`.
- `every_default_media_model_is_priced_or_local` (research R5).
- `pricer_media_order` — catalogue, then table tokens, then table unit, then unpriced; local is 0.
- `image_variant_falls_back_to_default`.
- Each provider client's `Reported` parsing against its golden fixture in `llm/media/fixtures/`.

## Live provider (marked: needs an OpenAI key, costs a few cents)

12. **Live, priced.** Agent with `image_gen` on OpenAI `gpt-image-1` and embeddings on OpenAI
    `text-embedding-3-small`. Generate one 1024×1024 image and write one memory. Report: the image
    row has `price_source: table` and `cost_usd` within 1% of OpenAI's published price for that size
    and quality (SC-002); the embedding row has `price_source: catalogue` and its tokens equal the
    response's `usage.prompt_tokens`.
