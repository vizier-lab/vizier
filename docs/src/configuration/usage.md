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

Costs are estimates, priced when the page is read, in this order:

1. **Local models are free.** On-device Whisper and Kokoro, the built-in local embeddings, Ollama
   and the offline `dummyplug` provider are shown as *free · local*, never as unpriced.
2. **The provider catalogue** (models.dev), for any call measured in tokens on a model it prices.
3. **Vizier's media price table**, shipped with each release
   (`crates/vizier-core/assets/media-prices.json`). It holds prices the catalogue cannot express,
   per image, per million characters and per audio minute, plus token rates for media models
   the catalogue does not list. Every entry names the provider's pricing page it was copied from
   and the date. On the page, an estimate from this table carries a note giving the table's date.
4. Anything else is **unpriced**: it is counted, and listed under *Not in the estimate* with its
   amount in its own unit.

The table only holds prices a provider publishes. Several default models had no published price
when the table was written, so they show as unpriced: OpenAI `dall-e-3`, Groq and Mistral
transcription, and the Hugging Face, Hyperbolic and Bedrock image models. A provider's price
change reaches the table with the next release.

## What is not counted

Model downloads (Ollama pulls, local model weights), the **Test** button on a provider, and work
that belongs to no agent.

## Before the upgrade

Media calls made before this version were never recorded, so a range that starts earlier shows no
media spend for those days. The report's `media_accounting_since` says when recording began.
