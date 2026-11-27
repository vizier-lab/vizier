# Usage and Spend

Every agent has a **Usage** page: what it spent on models in a period, where that went, and what
cost the most. It counts every call vizier makes on the agent's behalf:

- **Text**: every language-model completion. That includes chat turns, scheduled runs, dreams,
  checkpoint summaries, image reading and the rounds of turns that later failed.
- **Images**: every image the agent generates.
- **Speech**: every piece of speech it synthesizes, whether from the speech tool, a spoken reply or
  a live voice call.
- **Transcription**: every recording it transcribes, whether a voice message or speech in a live
  voice call.
- **Embeddings**: every embedding request for its memory and skills. That covers writes,
  searches, the automatic recall at the start of a turn, and re-indexing.

The page can be narrowed by **activity** (Chat, Scheduled, Dreams, …) and by **kind** (Text,
Images, Speech, Transcription, Embeddings), and broken down by activity, kind, model or channel.

## How media calls are counted

Each kind is counted in its own unit:

| Kind | Unit |
|---|---|
| Images | images (and the requested size) |
| Speech | characters sent, and seconds of audio when the audio is WAV or PCM |
| Transcription | seconds of audio heard, when the recording is WAV or the provider reports it |
| Embeddings | input tokens |

When the provider's response reports its own figures, vizier uses them. Today that means
OpenAI-compatible embeddings, OpenAI image tokens, and OpenAI transcription tokens or seconds.
Otherwise vizier measures the amount itself: one image a call, the characters of the text, the
length of a WAV file, and about four bytes a token for embeddings. A call with any measured amount
is marked **measured** in a conversation's details.

Embeddings made outside any conversation are counted under the **Indexing** activity. That covers
indexing an agent's memory when it starts, re-indexing after a change of embedding model, and
memory edits a person makes in the WebUI. Media calls made inside a conversation count under that
conversation's activity.

## Prices

Costs are estimates, priced when the page is read, so a newer price also re-prices past usage.
Every model row says where its price came from. Vizier asks, in this order:

1. **Free by rule.** A model on this machine (on-device Whisper and Kokoro, the built-in local
   embeddings, Ollama, llama.cpp and the offline `dummyplug` provider) is *free · local*. Usage on
   the ChatGPT subscription provider is *included · subscription*: $0 per call, because the
   subscription covers it. Neither is ever listed as unpriced.
2. **An inference-profile entry** (Amazon Bedrock over SSO): the price saved on the entry, then
   its paired model.
3. **The provider's own price list.** OpenRouter and Venice publish the price of every model they
   serve, text, image, speech and transcription alike, and Vizier ships both lists. A provider's
   list only ever prices that provider's own models: OpenRouter's list prices `openrouter`
   usage, never Anthropic's, even for a model with the same name.
4. **Vizier's price table**, shipped with each release
   (`crates/vizier-core/assets/media-prices.json`). It holds prices the other sources cannot
   express (per image, per million characters, per audio minute) and models no other source
   lists. Every entry names the provider page it was copied from and the date.
5. **models.dev**, the community catalogue, for every provider it lists.
6. **Borrowed.** A model with no price at its own provider (most often a custom provider in
   front of a well-known model) is priced at the same model id's price elsewhere, its maker
   first: `my-proxy / gpt-4o-mini` is priced at OpenAI's `gpt-4o-mini` rate. The row reads
   *estimate · borrowed from openai*, and the figure counts in the totals.
7. Anything else is **unpriced**: it is counted, and listed under *Not in the estimate* with its
   amount in its own unit. It is never shown as $0.

When two sources price the same model differently, the more authoritative one is used and the
row also shows what the other says.

The report's `price_source` field names the source or rule (`openrouter`, `venice`, `curated`,
`models_dev`, `inference_profile`, `local`, `subscription`, `borrowed` or `none`), and `sources`
gives each source's display name and date. Versions before spec 068 wrote `catalogue` for
models.dev and `table` for the price table. **This is a breaking change** for API clients that read
those two values.

A few default models still have no price from any source: OpenAI `dall-e-3`, Groq and Mistral
transcription, and the Hugging Face, Hyperbolic and Bedrock image models. They show as unpriced.

## What is not counted

Model downloads (Ollama pulls, local model weights), the **Test** button on a provider, and work
that belongs to no agent.

## Before the upgrade

Media calls made before this version were never recorded, so a range that starts earlier shows no
media spend for those days. The report's `media_accounting_since` says when recording began.
